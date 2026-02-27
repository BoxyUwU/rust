use derive_where::derive_where;
use indexmap::IndexSet;
#[cfg(feature = "nightly")]
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_data_structures::transitive_relation::{TransitiveRelation, TransitiveRelationBuilder};
use tracing::{debug, instrument};

use crate::data_structures::IndexMap;
use crate::fold::TypeSuperFoldable;
use crate::inherent::*;
use crate::relate::{Relate, RelateResult, TypeRelation, VarianceDiagInfo};
use crate::visit::TypeSuperVisitable;
use crate::{
    AliasTy, Binder, BoundRegion, BoundVar, BoundVariableKind, ConstKind, DebruijnIndex,
    FallibleTypeFolder, InferCtxtLike, InferTy, Interner, OutlivesPredicate, RegionKind, TyKind,
    TypeFoldable, TypeFolder, TypeVisitable, TypeVisitor, TypingMode, UniverseIndex, Variance,
    VisitorResult,
};

#[derive_where(Clone, Debug; I: Interner)]
pub struct Assumptions<I: Interner> {
    pub type_outlives: Vec<Binder<I, OutlivesPredicate<I, I::Ty>>>,
    pub region_outlives: TransitiveRelation<I::Region>,
    pub inverse_region_outlives: TransitiveRelation<I::Region>,
}

impl<I: Interner> Assumptions<I> {
    pub fn empty() -> Self {
        Self {
            type_outlives: Vec::new(),
            region_outlives: TransitiveRelationBuilder::default().freeze(),
            inverse_region_outlives: TransitiveRelationBuilder::default().freeze(),
        }
    }

    pub fn new(
        type_outlives: Vec<Binder<I, OutlivesPredicate<I, I::Ty>>>,
        region_outlives: TransitiveRelation<I::Region>,
    ) -> Self {
        Self {
            inverse_region_outlives: {
                let mut builder = TransitiveRelationBuilder::default();
                for (r1, r2) in region_outlives.base_edges() {
                    builder.add(r2, r1);
                }
                builder.freeze()
            },
            type_outlives,
            region_outlives,
        }
    }
}

#[derive_where(Clone, Hash, PartialEq, Debug; I: Interner)]
pub enum RegionConstraint<I: Interner> {
    Ambiguity,
    RegionOutlives(I::Region, I::Region),
    AliasTyOutlivesFromEnv(Binder<I, (AliasTy<I>, I::Region)>),
    /// This is an `I::Ty` for two reasons:
    /// 1. We need the type visitable impl to be able to `visit_ty` on this so canonicalization
    ///    knows about the placeholder
    /// 2. When exiting the trait solver there may be placeholder outlives corresponding to params
    ///    from the root universe. These need to be changed from a `Placeholder` to the original
    ///    `Param`.
    PlaceholderTyOutlives(I::Ty, I::Region),

    And(Box<[RegionConstraint<I>]>),
    Or(Box<[RegionConstraint<I>]>),
}

#[cfg(feature = "nightly")]
// This is not a derived impl because a perfect derive leads to cycle errors which
// means the trait is never actually implemented but the compiler doesn't tell you
// that so if you get a *WEIRD* error where its just telling you random types don't
// implement HashStable.... it's because of that
impl<CTX, I: Interner> HashStable<CTX> for RegionConstraint<I>
where
    I::Region: HashStable<CTX>,
    AliasTy<I>: HashStable<CTX>,
    I::Ty: HashStable<CTX>,
    I::BoundVarKinds: HashStable<CTX>,
{
    #[inline]
    fn hash_stable(&self, hcx: &mut CTX, hasher: &mut StableHasher) {
        use RegionConstraint::*;

        std::mem::discriminant(self).hash_stable(hcx, hasher);
        match self {
            Ambiguity => (),
            RegionOutlives(a, b) => {
                a.hash_stable(hcx, hasher);
                b.hash_stable(hcx, hasher);
            }
            AliasTyOutlivesFromEnv(outlives) => {
                outlives.hash_stable(hcx, hasher);
            }
            PlaceholderTyOutlives(a, b) => {
                a.hash_stable(hcx, hasher);
                b.hash_stable(hcx, hasher);
            }
            And(and) => {
                for a in and.iter() {
                    a.hash_stable(hcx, hasher);
                }
            }
            Or(or) => {
                for a in or.iter() {
                    a.hash_stable(hcx, hasher);
                }
            }
        }
    }
}

impl<I: Interner> TypeFoldable<I> for RegionConstraint<I> {
    fn try_fold_with<F: FallibleTypeFolder<I>>(self, f: &mut F) -> Result<Self, F::Error> {
        use RegionConstraint::*;
        Ok(match self {
            Ambiguity => self,
            RegionOutlives(a, b) => RegionOutlives(a.try_fold_with(f)?, b.try_fold_with(f)?),
            AliasTyOutlivesFromEnv(outlives) => AliasTyOutlivesFromEnv(outlives.try_fold_with(f)?),
            PlaceholderTyOutlives(a, b) => {
                PlaceholderTyOutlives(a.try_fold_with(f)?, b.try_fold_with(f)?)
            }
            And(and) => {
                let mut new_and = Vec::new();
                for a in and {
                    new_and.push(a.try_fold_with(f)?);
                }
                And(new_and.into_boxed_slice())
            }
            Or(or) => {
                let mut new_or = Vec::new();
                for a in or {
                    new_or.push(a.try_fold_with(f)?);
                }
                Or(new_or.into_boxed_slice())
            }
        })
    }

    fn fold_with<F: TypeFolder<I>>(self, f: &mut F) -> Self {
        use RegionConstraint::*;
        match self {
            Ambiguity => self,
            RegionOutlives(a, b) => RegionOutlives(a.fold_with(f), b.fold_with(f)),
            AliasTyOutlivesFromEnv(outlives) => AliasTyOutlivesFromEnv(outlives.fold_with(f)),
            PlaceholderTyOutlives(a, b) => PlaceholderTyOutlives(a.fold_with(f), b.fold_with(f)),
            And(and) => {
                let mut new_and = Vec::new();
                for a in and {
                    new_and.push(a.fold_with(f));
                }
                And(new_and.into_boxed_slice())
            }
            Or(or) => {
                let mut new_or = Vec::new();
                for a in or {
                    new_or.push(a.fold_with(f));
                }
                Or(new_or.into_boxed_slice())
            }
        }
    }
}

impl<I: Interner> TypeVisitable<I> for RegionConstraint<I> {
    fn visit_with<F: TypeVisitor<I>>(&self, f: &mut F) -> F::Result {
        use core::ops::ControlFlow::*;

        use RegionConstraint::*;

        match self {
            Ambiguity => (),
            RegionOutlives(a, b) => {
                if let b @ Break(_) = a.visit_with(f).branch() {
                    return F::Result::from_branch(b);
                };
                if let b @ Break(_) = b.visit_with(f).branch() {
                    return F::Result::from_branch(b);
                };
            }
            AliasTyOutlivesFromEnv(outlives) => {
                return outlives.visit_with(f);
            }
            PlaceholderTyOutlives(a, b) => {
                if let b @ Break(_) = a.visit_with(f).branch() {
                    return F::Result::from_branch(b);
                };
                if let b @ Break(_) = b.visit_with(f).branch() {
                    return F::Result::from_branch(b);
                };
            }
            And(and) => {
                for a in and {
                    if let b @ Break(_) = a.visit_with(f).branch() {
                        return F::Result::from_branch(b);
                    };
                }
            }
            Or(or) => {
                for a in or {
                    if let b @ Break(_) = a.visit_with(f).branch() {
                        return F::Result::from_branch(b);
                    };
                }
            }
        };

        F::Result::output()
    }
}

impl<I: Interner> Default for RegionConstraint<I> {
    fn default() -> Self {
        Self::new_true()
    }
}

impl<I: Interner> RegionConstraint<I> {
    pub fn new_true() -> Self {
        RegionConstraint::And(Box::new([]))
    }

    pub fn is_true(&self) -> bool {
        match self {
            Self::And(and) => and.is_empty(),
            _ => false,
        }
    }

    pub fn new_false() -> Self {
        RegionConstraint::Or(Box::new([]))
    }

    pub fn is_false(&self) -> bool {
        match self {
            Self::Or(or) => or.is_empty(),
            _ => false,
        }
    }

    pub fn and(self, other: RegionConstraint<I>) -> RegionConstraint<I> {
        use RegionConstraint::*;

        match (self, other) {
            (And(a_ands), And(b_ands)) => And(a_ands
                .into_iter()
                .chain(b_ands.into_iter())
                .collect::<Vec<_>>()
                .into_boxed_slice()),
            (And(ands), other) | (other, And(ands)) => {
                And(ands.into_iter().chain([other]).collect::<Vec<_>>().into_boxed_slice())
            }
            (this, other) => And(Box::new([this, other])),
        }
    }

    pub fn is_or(&self) -> bool {
        matches!(self, Self::Or(_))
    }

    pub fn is_ambig(&self) -> bool {
        matches!(self, Self::Ambiguity)
    }

    #[instrument(level = "debug", ret)]
    pub fn canonical_form(self) -> Self {
        use RegionConstraint::*;

        fn permutations<I: Interner>(ors: &[Vec<RegionConstraint<I>>]) -> Vec<RegionConstraint<I>> {
            match ors {
                [] => vec![],
                [or] => or.clone(),
                [or1, or2] => {
                    let mut permutations = vec![];
                    for c1 in or1 {
                        for c2 in or2 {
                            permutations.push(c1.clone().and(c2.clone()));
                        }
                    }

                    permutations
                }
                [rest @ .., or1, or2] => {
                    let combined_or = permutations(&[or1.clone(), or2.clone()]);

                    let mut input = vec![];
                    input.push(combined_or);
                    input.extend(rest.to_vec());
                    permutations(&input)
                }
            }
        }

        let canonical = match self {
            And(ands) => {
                let mut un_ored = vec![];
                let mut ors = vec![];

                let mut temp_ands: Vec<_> = ands.into();
                while let Some(c) = temp_ands.pop() {
                    let c = c.canonical_form();

                    if let Or(c_ors) = c {
                        ors.push(c_ors.into());
                    } else if let And(ands) = c {
                        temp_ands.extend(ands);
                    } else {
                        un_ored.push(c);
                    }
                }

                let mut or_combinations = permutations(&ors);
                match or_combinations.len() {
                    0 => And(un_ored.into_boxed_slice()),
                    1 => And(un_ored.into_boxed_slice()).and(or_combinations.pop().unwrap()),
                    _ => Or(or_combinations
                        .into_iter()
                        .map(|c| And(un_ored.clone().into_boxed_slice()).and(c))
                        .collect::<Vec<_>>()
                        .into_boxed_slice()),
                }
            }
            Or(ors) => {
                let mut constraints = vec![];

                let mut temp_ors: Vec<_> = ors.into();
                while let Some(c) = temp_ors.pop() {
                    let c = c.canonical_form();
                    if let Or(c_ors) = c {
                        temp_ors.extend(c_ors);
                    } else {
                        constraints.push(c);
                    }
                }

                if constraints.len() == 1 {
                    constraints.pop().unwrap()
                } else {
                    Or(constraints.into_boxed_slice())
                }
            }
            _ => self,
        };

        assert!(canonical.is_canonical_form());
        canonical
    }

    fn is_leaf_constraint(&self) -> bool {
        use RegionConstraint::*;
        match self {
            Ambiguity
            | RegionOutlives(..)
            | AliasTyOutlivesFromEnv(..)
            | PlaceholderTyOutlives(..) => true,
            And(..) | Or(..) => false,
        }
    }

    fn is_and_of_leaf_constraints(&self) -> bool {
        if let Self::And(ands) = self { ands.iter().all(|c| c.is_leaf_constraint()) } else { false }
    }

    fn is_or_of_and_of_leaf_constraints(&self) -> bool {
        if let Self::Or(ors) = self {
            ors.iter().all(|c| c.is_leaf_constraint() || c.is_and_of_leaf_constraints())
        } else {
            false
        }
    }

    pub fn is_canonical_form(&self) -> bool {
        self.is_leaf_constraint()
            || self.is_and_of_leaf_constraints()
            || self.is_or_of_and_of_leaf_constraints()
    }
}

impl<I: Interner> From<bool> for RegionConstraint<I> {
    fn from(b: bool) -> Self {
        match b {
            true => Self::new_true(),
            false => Self::new_false(),
        }
    }
}

#[instrument(level = "debug", skip(infcx), ret)]
pub fn eagerly_handle_placeholders_in_universe<Infcx: InferCtxtLike<Interner = I>, I: Interner>(
    infcx: &Infcx,
    constraint: RegionConstraint<I>,
    u: UniverseIndex,
) -> RegionConstraint<I> {
    use RegionConstraint::*;

    let assumptions = infcx.get_universe_assumptions(u);

    // 1. rewrite type outlives constraints
    let constraint =
        destructure_type_outlives_constraints_in_universe(infcx, constraint, Some(u), &assumptions);

    // 2. rewrite the constraint into a canonical ORs of ANDs form
    let constraint = constraint.canonical_form();

    // 3. compute transitive region outlives and get a new set of region outlives constraints by
    //     looking for every region which either a placeholder_u flows into it, or it flows into
    //     the placeholder.
    //
    //    do this for each element in the top level OR
    let constraint = match constraint {
        Or(ors) => {
            let new_ors = ors.into_iter().map(|c| match c {
                And(ands) => {
                    And(compute_new_region_constraints(infcx, &ands, u).into_boxed_slice())
                }
                Or(_) => unreachable!(),
                _ => {
                    let mut constraints = compute_new_region_constraints(infcx, &[c], u);
                    assert!(constraints.len() == 1);
                    constraints.pop().unwrap()
                }
            });
            Or(new_ors.collect::<Vec<_>>().into_boxed_slice())
        }
        And(ands) => And(compute_new_region_constraints(infcx, &ands, u).into_boxed_slice()),
        _ => {
            let mut constraints = compute_new_region_constraints(infcx, &[constraint], u);
            assert!(constraints.len() == 1);
            constraints.pop().unwrap()
        }
    };

    // 4. rewrite region outlives constraints (potentially to false/true)
    let constraint = pull_region_constraint_out_of_universe(infcx, constraint, u, &assumptions);

    // 5. actually evalaute the constraint to eagerly error on false
    evaluate_solver_constraint(&constraint)
}

#[instrument(level = "debug", skip(infcx), ret)]
fn compute_new_region_constraints<Infcx: InferCtxtLike<Interner = I>, I: Interner>(
    infcx: &Infcx,
    constraints: &[RegionConstraint<I>],
    u: UniverseIndex,
) -> Vec<RegionConstraint<I>> {
    use RegionConstraint::*;

    let mut new_constraints = vec![];

    let mut region_flows_builder = TransitiveRelationBuilder::default();
    let mut regions = IndexSet::new();
    for c in constraints {
        match c {
            And(..) | Or(..) => unreachable!(),
            Ambiguity | PlaceholderTyOutlives(..) | AliasTyOutlivesFromEnv(..) => {
                new_constraints.push(c.clone())
            }
            RegionOutlives(r1, r2) => {
                regions.insert(r1);
                regions.insert(r2);
                region_flows_builder.add(r2, r1);
            }
        }
    }

    let region_flow = region_flows_builder.freeze();
    for r in regions.into_iter() {
        for ub in region_flow.reachable_from(r) {
            // we want to retain any region constraints between two "placeholder-likes" where for our
            // purposes a placeholder-like is either a placeholder or variable in a lower universe
            let is_placeholder_like = |r: I::Region| match r.kind() {
                RegionKind::ReLateParam(..)
                | RegionKind::ReEarlyParam(..)
                | RegionKind::RePlaceholder(..)
                | RegionKind::ReStatic => true,
                RegionKind::ReVar(..) => max_universe(infcx, r) < u,
                RegionKind::ReError(..) | RegionKind::ReErased => false,
                RegionKind::ReBound(..) => unreachable!(),
            };

            if is_placeholder_like(*r) && is_placeholder_like(*ub) {
                new_constraints.push(RegionOutlives(*ub, *r));
            }
        }
    }

    new_constraints
}

#[derive(Copy, Clone, Debug)]
enum Certainty {
    Yes,
    Ambig,
}

#[instrument(level = "debug", ret)]
pub fn evaluate_solver_constraint<I: Interner>(
    constraint: &RegionConstraint<I>,
) -> RegionConstraint<I> {
    use RegionConstraint::*;
    match constraint {
        Ambiguity | RegionOutlives(..) | AliasTyOutlivesFromEnv(..) | PlaceholderTyOutlives(..) => {
            constraint.clone()
        }
        And(and) => {
            let mut and_constraints = Vec::new();
            let mut certainty = Certainty::Yes;
            for c in and.iter() {
                let evaluated_constraint = evaluate_solver_constraint(c);
                if evaluated_constraint.is_true() {
                    // - do nothing
                } else if evaluated_constraint.is_false() {
                    and_constraints = vec![RegionConstraint::new_false()];
                    certainty = Certainty::Yes;
                    break;
                } else {
                    if evaluated_constraint.is_ambig() {
                        certainty = Certainty::Ambig;
                    }
                    and_constraints.push(evaluated_constraint);
                }
            }

            if let Certainty::Ambig = certainty {
                RegionConstraint::Ambiguity
            } else if and_constraints.len() == 1 {
                and_constraints.pop().unwrap()
            } else {
                RegionConstraint::And(and_constraints.into_boxed_slice())
            }
        }
        Or(or) => {
            let mut or_constraints = Vec::new();
            let mut certainty = Certainty::Yes;
            for c in or.iter() {
                let evaluated_constraint = evaluate_solver_constraint(c);
                if evaluated_constraint.is_false() {
                    // do nothing
                } else if evaluated_constraint.is_true() {
                    or_constraints = vec![RegionConstraint::new_true()];
                    certainty = Certainty::Yes;
                    break;
                } else {
                    if evaluated_constraint.is_ambig() {
                        certainty = Certainty::Ambig;
                    }
                    or_constraints.push(evaluated_constraint);
                }
            }

            if let Certainty::Ambig = certainty {
                RegionConstraint::Ambiguity
            } else if or_constraints.len() == 1 {
                or_constraints.pop().unwrap()
            } else {
                RegionConstraint::Or(or_constraints.into_boxed_slice())
            }
        }
    }
}

#[instrument(level = "debug", skip(infcx), ret)]
fn pull_region_constraint_out_of_universe<Infcx: InferCtxtLike<Interner = I>, I: Interner>(
    infcx: &Infcx,
    constraint: RegionConstraint<I>,
    u: UniverseIndex,
    assumptions: &Option<Assumptions<I>>,
) -> RegionConstraint<I> {
    assert!(max_universe(infcx, constraint.clone()) <= u);

    // FIXME(-Zhigher-ranked-assumptions-v2): we don't lower universes of region variables when exiting `u`
    // this seems dubious/potentially wrong? we can't just blindly do this though as if we had something
    // like `!T_u -> ?x_u -> !U_u` then lowering `?x` to `u-1` when exiting `u` would be wrong.

    use RegionConstraint::*;
    match constraint {
        Ambiguity | PlaceholderTyOutlives(..) | AliasTyOutlivesFromEnv(..) => {
            assert!(max_universe(infcx, constraint.clone()) < u);
            constraint
        }
        RegionOutlives(region_1, region_2) => {
            let region_1_u = max_universe(infcx, region_1);
            let region_2_u = max_universe(infcx, region_2);

            if region_1_u != u && region_2_u != u {
                return constraint;
            }

            let assumptions = match assumptions {
                Some(assumptions) => assumptions,
                None => return RegionConstraint::Ambiguity,
            };

            if regions_outliving::<I>(region_2, assumptions, infcx.cx())
                .find(|r| *r == region_1)
                .is_some()
            {
                return RegionConstraint::new_true();
            }

            let mut constraints = vec![RegionOutlives::<I>(region_1, region_2)];
            // `'r1_Uu: x`
            if region_1_u == u {
                // all regions `'y` for which `'r1_Um: 'y_Un` where `n < m`
                constraints = regions_outlived_by::<I>(region_1, assumptions)
                    .filter(|r| max_universe(infcx, *r) < region_1_u)
                    .map(|r| RegionOutlives(r, region_2))
                    .collect();
            }

            // `'x: 'r2_Uu`
            if region_2_u == u {
                constraints = constraints
                    .into_iter()
                    .flat_map(|constraint| {
                        let RegionOutlives(region_1, _) = constraint else { unreachable!() };
                        // all regions `'y` for which `'y_Un: 'r2_Um` where `n < m`
                        regions_outliving::<I>(region_2, assumptions, infcx.cx())
                            .filter(|r| max_universe(infcx, *r) < region_2_u)
                            .map(move |r| RegionOutlives::<I>(region_1, r))
                    })
                    .collect();
            }

            RegionConstraint::Or(constraints.into_boxed_slice())
        }
        And(constraints) => And(constraints
            .into_iter()
            .map(|constraint| {
                pull_region_constraint_out_of_universe(infcx, constraint, u, assumptions)
            })
            .collect()),
        Or(constraints) => Or(constraints
            .into_iter()
            .map(|constraint| {
                pull_region_constraint_out_of_universe(infcx, constraint, u, assumptions)
            })
            .collect()),
    }
}

#[instrument(level = "debug", skip(infcx), ret)]
pub fn destructure_type_outlives_constraints_in_universe<
    Infcx: InferCtxtLike<Interner = I>,
    I: Interner,
>(
    infcx: &Infcx,
    constraint: RegionConstraint<I>,
    u: Option<UniverseIndex>,
    assumptions: &Option<Assumptions<I>>,
) -> RegionConstraint<I> {
    if let Some(u) = u {
        assert!(
            max_universe(infcx, constraint.clone()) <= u,
            "constraint {:?} contains terms from a larger universe than {:?}",
            constraint.clone(),
            u
        );
    }

    use RegionConstraint::*;
    match constraint {
        Ambiguity | RegionOutlives(..) => constraint,
        PlaceholderTyOutlives(ty, region) => {
            let ty_u = max_universe(infcx, ty);
            let region_u = max_universe(infcx, region);

            if u.is_some_and(|u| region_u != u && ty_u != u) {
                return constraint;
            }

            let assumptions = match assumptions {
                Some(assumptions) => assumptions,
                None => return RegionConstraint::Ambiguity,
            };

            // FIXME(-Zhigher-ranked-assumptions-v2): things are slightly wrong here, if we know `!T_u1: '!a_u1` and
            // `'!a_u1: '!b_u1` and are rewriting `!T: '!b_u1` then that should probably succeed, but we don't handle
            // that here. IOW type outlives assumptions aren't treated transitively but they should be

            if regions_outlived_by_placeholder::<I>(ty, assumptions, infcx.cx())
                .find(|r| *r == region)
                .is_some()
            {
                debug!("matched assumption for ty outlives");
                return RegionConstraint::new_true();
            }

            let mut candidates = vec![PlaceholderTyOutlives(ty, region)];

            // transitive outlives involving the region, e.g. `!T: 'r_Uu` can be rewritten to `!T: 'x_Uu-1` if `'x_Uu-1: 'r_Uu` is known
            // we don't really care about this if `u` is `None` because we just want a big OR constraint of outlives between all assumptions
            if u.is_some_and(|u| region_u == u) {
                // all regions `'y` for which `'y_Un: 'r_Uu` where `n < u`
                candidates = regions_outliving::<I>(region, assumptions, infcx.cx())
                    .filter(|r| max_universe(infcx, *r) < region_u)
                    .map(move |r| PlaceholderTyOutlives::<I>(ty, r))
                    .collect();
            }

            // assumptions on `!T`, e.g. `!T: 'x_Uu-1` should result in a `'r_Uu: 'x_Uu-1` constraint
            if u.is_none_or(|u| ty_u == u) {
                candidates = candidates
                    .into_iter()
                    .flat_map(|constraint| {
                        let PlaceholderTyOutlives(ty, region) = constraint else { unreachable!() };

                        regions_outlived_by_placeholder::<I>(ty, assumptions, infcx.cx())
                            .filter(|r| u.is_none_or(|u| max_universe(infcx, *r) < u))
                            .map(move |r| RegionOutlives(r, region))
                    })
                    .collect();
            }

            RegionConstraint::Or(candidates.into_boxed_slice())
        }
        AliasTyOutlivesFromEnv(bound_outlives) => {
            let mut candidates = Vec::new();

            // Actually look at the assumptions and matching our higher ranked alias outlives goal
            // against potentially higher ranked type outlives assumptions.
            match assumptions {
                opt_assumptions @ Some(assumptions) => {
                    let requirements =
                        alias_outlives_candidate_requirement(infcx, bound_outlives, assumptions);
                    let rewritten_requiurements = destructure_type_outlives_constraints_in_universe(
                        infcx,
                        requirements,
                        u,
                        opt_assumptions,
                    );
                    candidates.push(rewritten_requiurements);
                }
                None => candidates.push(RegionConstraint::Ambiguity),
            };

            // given there can be higher ranked assumptions, e.g. `for<'a> <T as Trait<'a>>::Assoc: 'c`, that
            // means that it's actually *always* possible for an alias outlive to be satisfied in the root universe
            // which means there should *always* be atleast two candidates when destructuring alias outlives. The
            // two candidates being component outlives and then a higher ranked alias outlives.
            //
            // we dont care about this for region outlives as `for<'a> 'a: 'b` can't exist as we don't elaborate
            // higher ranked type outlives assumptions into higher ranked region outlives assumptions. similarly,
            // we don't care about `for<'a> Foo<'a>: 'b` as we always destructure adts into their components and if
            // we dont equivalently elaborate the assumption into assumptions on the adt's components we just drop the
            // assumptions
            //
            // so actually only `for<'a, 'b> Alias<'a>: 'b` and `for<'a> T: 'a` are assumptions we actually need to
            // handle.
            //
            // we don't care about this when rewriting in the root universe as we know the complete set of assumptions
            if let Some(u) = u
                && max_universe(infcx, bound_outlives) == u
            {
                let mut replacer = PlaceholderReplacer {
                    cx: infcx.cx(),
                    existing_var_count: bound_outlives.bound_vars().len(),
                    bound_vars: IndexMap::default(),
                    universe: u,
                    current_index: DebruijnIndex::ZERO,
                };
                let escaping_outlives = bound_outlives.skip_binder().fold_with(&mut replacer);
                let bound_vars = bound_outlives.bound_vars().iter().chain(
                    core::mem::take(&mut replacer.bound_vars)
                        .into_iter()
                        .map(|(_, bound_region)| BoundVariableKind::Region(bound_region.kind)),
                );
                let bound_outlives = Binder::bind_with_vars(
                    escaping_outlives,
                    I::BoundVarKinds::from_vars(infcx.cx(), bound_vars),
                );
                candidates.push(RegionConstraint::AliasTyOutlivesFromEnv(bound_outlives));
            }

            // we can rewrite `Alias_u1: 'u2` into `Or(Alias_u1: 'u1)`
            // given a list of regions which outlive `'u2`
            //
            // we don't care about this when rewriting in the root universe as we know the complete set of assumptions
            let (escaping_alias, escaping_r) = bound_outlives.skip_binder();
            if let Some(u) = u
                && max_universe(infcx, escaping_r) == u
            {
                match assumptions {
                    Some(assumptions) => {
                        let mut replacer = PlaceholderReplacer {
                            cx: infcx.cx(),
                            existing_var_count: bound_outlives.bound_vars().len(),
                            bound_vars: IndexMap::default(),
                            universe: u,
                            current_index: DebruijnIndex::ZERO,
                        };
                        let escaping_alias = escaping_alias.fold_with(&mut replacer);
                        let bound_vars = bound_outlives.bound_vars().iter().chain(
                            core::mem::take(&mut replacer.bound_vars).into_iter().map(
                                |(_, bound_region)| BoundVariableKind::Region(bound_region.kind),
                            ),
                        );
                        let bound_alias = Binder::bind_with_vars(
                            escaping_alias,
                            I::BoundVarKinds::from_vars(infcx.cx(), bound_vars),
                        );

                        // while we did skip the binder, bound vars aren't in any universe so
                        // this can't be an escaping bound var
                        candidates.extend(
                            regions_outliving(escaping_r, assumptions, infcx.cx())
                                .filter(|r2| max_universe(infcx, *r2) < u)
                                .map(|r2| {
                                    AliasTyOutlivesFromEnv(
                                        bound_alias.map_bound(|alias| (alias, r2)),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        );
                    }
                    None => candidates.push(RegionConstraint::Ambiguity),
                };
            }

            // I'm not convinced our handling here is *complete* so for now
            // let's be conservative and not let alias outlives' cause leak check
            // errors in coherence
            match infcx.typing_mode() {
                TypingMode::Coherence => candidates.push(RegionConstraint::Ambiguity),
                TypingMode::Analysis { .. }
                | TypingMode::Borrowck { .. }
                | TypingMode::PostBorrowckAnalysis { .. }
                | TypingMode::PostAnalysis => (),
            };

            RegionConstraint::Or(candidates.into_boxed_slice())
        }
        And(constraints) => And(constraints
            .into_iter()
            .map(|constraint| {
                destructure_type_outlives_constraints_in_universe(infcx, constraint, u, assumptions)
            })
            .collect()),
        Or(constraints) => Or(constraints
            .into_iter()
            .map(|constraint| {
                destructure_type_outlives_constraints_in_universe(infcx, constraint, u, assumptions)
            })
            .collect()),
    }
}

pub fn regions_outlived_by<I: Interner>(
    r: I::Region,
    assumptions: &Assumptions<I>,
) -> impl Iterator<Item = I::Region> {
    assumptions.region_outlives.reachable_from(r).into_iter()
}

pub fn regions_outliving<I: Interner>(
    r: I::Region,
    assumptions: &Assumptions<I>,
    cx: I,
) -> impl Iterator<Item = I::Region> {
    // FIXME: 'static may have been an input region canonicalized to something else is that important?
    assumptions
        .inverse_region_outlives
        .reachable_from(r)
        .into_iter()
        .chain([I::Region::new_static(cx)])
}

pub fn regions_outlived_by_placeholder<I: Interner>(
    t: I::Ty,
    assumptions: &Assumptions<I>,
    cx: I,
) -> impl Iterator<Item = I::Region> {
    match t.kind() {
        TyKind::Placeholder(..) | TyKind::Param(..) => (),
        _ => unreachable!("non-placeholder in `regions_outlived_by_placeholder`: {t:?}"),
    }

    assumptions.type_outlives.iter().flat_map(move |binder| match binder.no_bound_vars() {
        Some(OutlivesPredicate(ty, r)) => (ty == t).then_some(r),
        None => Some(I::Region::new_static(cx)),
    })
}

pub fn max_universe<Infcx: InferCtxtLike<Interner = I>, I: Interner, T: TypeVisitable<I>>(
    infcx: &Infcx,
    t: T,
) -> UniverseIndex {
    let mut visitor = MaxUniverse::new(infcx);
    t.visit_with(&mut visitor);
    visitor.max_universe()
}

struct MaxUniverse<'a, Infcx: InferCtxtLike> {
    max_universe: UniverseIndex,
    infcx: &'a Infcx,
}

impl<'a, Infcx: InferCtxtLike> MaxUniverse<'a, Infcx> {
    fn new(infcx: &'a Infcx) -> Self {
        MaxUniverse { infcx, max_universe: UniverseIndex::ROOT }
    }

    fn max_universe(self) -> UniverseIndex {
        self.max_universe
    }
}

impl<'a, Infcx: InferCtxtLike<Interner = I>, I: Interner> TypeVisitor<I>
    for MaxUniverse<'a, Infcx>
{
    fn visit_ty(&mut self, t: I::Ty) {
        if let TyKind::Placeholder(placeholder) = t.kind() {
            self.max_universe = self.max_universe.max(placeholder.universe);
        }

        if let TyKind::Infer(InferTy::TyVar(inf)) = t.kind() {
            let u = self.infcx.universe_of_ty(inf).unwrap();
            debug!("var {inf:?} in universe {u:?}");
            self.max_universe = self.max_universe.max(u)
        }

        t.super_visit_with(self)
    }

    fn visit_const(&mut self, c: I::Const) {
        if let ConstKind::Placeholder(placeholder) = c.kind() {
            self.max_universe = self.max_universe.max(placeholder.universe);
        }

        if let ConstKind::Infer(rustc_type_ir::InferConst::Var(inf)) = c.kind() {
            let u = self.infcx.universe_of_ct(inf).unwrap();
            debug!("var {inf:?} in universe {u:?}");
            self.max_universe = self.max_universe.max(u)
        }

        c.super_visit_with(self)
    }

    fn visit_region(&mut self, r: I::Region) {
        if let RegionKind::RePlaceholder(placeholder) = r.kind() {
            self.max_universe = self.max_universe.max(placeholder.universe);
        }

        if let RegionKind::ReVar(var) = r.kind() {
            let u = self.infcx.universe_of_lt(var).unwrap();
            debug!("var {var:?} in universe {u:?}");
            self.max_universe = self.max_universe.max(u)
        }
    }
}

pub struct PlaceholderReplacer<I: Interner> {
    cx: I,
    existing_var_count: usize,
    bound_vars: IndexMap<BoundVar, BoundRegion<I>>,
    universe: UniverseIndex,
    current_index: DebruijnIndex,
}

impl<I: Interner> TypeFolder<I> for PlaceholderReplacer<I> {
    fn cx(&self) -> I {
        self.cx
    }

    fn fold_region(&mut self, r: I::Region) -> I::Region {
        match r.kind() {
            RegionKind::RePlaceholder(p) if p.universe == self.universe => {
                let bound_vars_len = self.bound_vars.len();
                let mapped_var = self.bound_vars.entry(p.bound.var).or_insert(BoundRegion {
                    var: BoundVar::from_usize(self.existing_var_count + bound_vars_len),
                    kind: p.bound.kind,
                });
                I::Region::new_bound(self.cx, self.current_index, *mapped_var)
            }
            _ => r,
        }
    }

    fn fold_binder<T: TypeFoldable<I>>(&mut self, b: Binder<I, T>) -> Binder<I, T> {
        self.current_index.shift_in(1);
        let b = b.super_fold_with(self);
        self.current_index.shift_out(1);
        b
    }
}

#[instrument(level = "debug", skip(infcx), ret)]
pub fn alias_outlives_candidate_requirement<Infcx: InferCtxtLike<Interner = I>, I: Interner>(
    infcx: &Infcx,
    bound_outlives: Binder<I, (AliasTy<I>, I::Region)>,
    assumptions: &Assumptions<I>,
) -> RegionConstraint<I> {
    let mut candidates = Vec::new();

    let prev_universe = infcx.universe();

    infcx.enter_forall(bound_outlives, |(alias, r)| {
        let u = infcx.universe();
        infcx.insert_universe_assumptions(u, Some(Assumptions::empty()));

        for bound_type_outlives in assumptions.type_outlives.iter() {
            let OutlivesPredicate(alias2, r2) =
                infcx.instantiate_binder_with_infer(*bound_type_outlives);

            let mut relation = HigherRankedAliasMatcher {
                infcx,
                region_constraints: vec![RegionConstraint::RegionOutlives(r2, r)],
            };

            if let Ok(_) = relation.relate(alias.to_ty(infcx.cx()), alias2) {
                candidates
                    .push(RegionConstraint::And(relation.region_constraints.into_boxed_slice()));
            }
        }
    });

    let constraint = RegionConstraint::Or(candidates.into_boxed_slice());

    let largest_universe = infcx.universe();
    debug!(?prev_universe, ?largest_universe);

    ((prev_universe.index() + 1)..=largest_universe.index())
        .map(|u| UniverseIndex::from_usize(u))
        .rev()
        .fold(constraint, |constraint, u| {
            eagerly_handle_placeholders_in_universe(infcx, constraint, u)
        })
}

struct HigherRankedAliasMatcher<'a, Infcx: InferCtxtLike<Interner = I>, I: Interner> {
    infcx: &'a Infcx,
    region_constraints: Vec<RegionConstraint<I>>,
}

impl<'a, Infcx: InferCtxtLike<Interner = I>, I: Interner> TypeRelation<I>
    for HigherRankedAliasMatcher<'a, Infcx, I>
{
    fn cx(&self) -> I {
        self.infcx.cx()
    }

    fn relate_ty_args(
        &mut self,
        a_ty: I::Ty,
        _b_ty: I::Ty,
        _ty_def_id: I::DefId,
        a_args: I::GenericArgs,
        b_args: I::GenericArgs,
        _mk: impl FnOnce(I::GenericArgs) -> I::Ty,
    ) -> RelateResult<I, I::Ty> {
        rustc_type_ir::relate::relate_args_invariantly(self, a_args, b_args)?;
        Ok(a_ty)
    }

    fn relate_with_variance<T: Relate<I>>(
        &mut self,
        _variance: Variance,
        _info: VarianceDiagInfo<I>,
        a: T,
        b: T,
    ) -> RelateResult<I, T> {
        // FIXME(-Zhigher-ranked-assumptions-v2): bivariance is important for opaque type args so
        // we should actually handle variance in some way here.
        self.relate(a, b)
    }

    fn tys(&mut self, a: I::Ty, b: I::Ty) -> RelateResult<I, I::Ty> {
        rustc_type_ir::relate::structurally_relate_tys(self, a, b)
    }

    fn regions(&mut self, a: I::Region, b: I::Region) -> RelateResult<I, I::Region> {
        if a != b {
            self.region_constraints.push(RegionConstraint::RegionOutlives(a, b));
            self.region_constraints.push(RegionConstraint::RegionOutlives(b, a));
        }
        Ok(a)
    }

    fn consts(&mut self, a: I::Const, b: I::Const) -> RelateResult<I, I::Const> {
        rustc_type_ir::relate::structurally_relate_consts(self, a, b)
    }

    fn binders<T>(&mut self, a: Binder<I, T>, b: Binder<I, T>) -> RelateResult<I, Binder<I, T>>
    where
        T: Relate<I>,
    {
        self.infcx.enter_forall(a, |a| {
            let u = self.infcx.universe();
            self.infcx.insert_universe_assumptions(u, Some(Assumptions::empty()));
            let b = self.infcx.instantiate_binder_with_infer(b);
            self.relate(a, b)
        })?;

        self.infcx.enter_forall(b, |b| {
            let u = self.infcx.universe();
            self.infcx.insert_universe_assumptions(u, Some(Assumptions::empty()));
            let a = self.infcx.instantiate_binder_with_infer(a);
            self.relate(a, b)
        })?;

        Ok(a)
    }
}
