use std::ops::ControlFlow;

use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::DefId;
use rustc_infer::{
    infer::{
        type_variable::{TypeVariableOrigin, TypeVariableOriginKind},
        InferCtxt, RegionVariableOrigin, TyCtxtInferExt,
    },
    traits::{query::NoSolution, Obligation, ObligationCause, Reveal},
};
use rustc_middle::{
    infer::unify_key::{ConstVariableOrigin, ConstVariableOriginKind},
    ty::{
        self, Const, ConstKind, List, ParamEnv, Predicate, Region, RegionKind, Ty, TyCtxt,
        TypeFoldable, TypeFolder, TypeSuperFoldable, TypeSuperVisitable, TypeVisitable,
        TypeVisitor,
    },
};
use rustc_span::DUMMY_SP;

use crate::traits::normalize_param_env_or_error;

use super::ObligationCtxt;

#[instrument(level = "debug", skip(tcx), ret)]
pub fn self_ty_and_predicates_for_synthetic_auto_trait_impl<'tcx>(
    tcx: TyCtxt<'tcx>,
    early_binder_self_ty: ty::EarlyBinder<Ty<'tcx>>,
    auto_trait: DefId,
) -> Result<(Ty<'tcx>, &'tcx List<Predicate<'tcx>>), NoSolution> {
    // auto traits cannot have any generics other than the self ty but if we were to allow having
    // generics on auto traits then this code would be wrong so assert it.
    assert!(tcx.generics_of(auto_trait).count() == 1);

    let infcx = tcx.infer_ctxt().build();

    let (self_ty, param_to_infers) = {
        let mut folder =
            InstantiateEarlyBinderWithInfer { infcx: &infcx, mapping: Default::default() };
        (early_binder_self_ty.skip_binder().fold_with(&mut folder), folder.mapping)
    };
    debug!("instantiated `self_ty`: {:?}", self_ty);
    let allowed_infers = {
        let mut visitor = InferVarCollector::default();
        self_ty.visit_with(&mut visitor);
        visitor
    };

    let mut synthetic_impl_preds = Vec::new();
    let dummy_cause = ObligationCause::dummy();

    let mut iterations = 0;
    let result = loop {
        debug!("starting iteration n={}", iterations);
        iterations += 1;
        // ensure that we do not enter an infinite loop or iterate for "too long"
        // if we do fallback to a simple set of where clauses and substs that does not
        // require looping.
        if !tcx.recursion_limit().value_within_limit(iterations - 1) {
            debug!(
                "hit recursion limit when trying to determine bounds on synthetic impl for `{:?}: {:?}`",
                early_binder_self_ty, auto_trait
            );
            return naive_self_ty_and_predicates_for_synthetic_auto_trait_impl(
                tcx,
                early_binder_self_ty,
                auto_trait,
            );
        }

        let a = crate::traits::util::elaborate_predicates(tcx, synthetic_impl_preds.into_iter())
            .map(|o| o.predicate)
            .collect::<Vec<_>>();
        let a_param_env = ParamEnv::new(
            tcx.mk_predicates(&a),
            Reveal::UserFacing,
            rustc_hir::Constness::NotConst,
        );
        let a =
            crate::traits::fully_normalize(&infcx, dummy_cause.clone(), a_param_env, a).unwrap();
        synthetic_impl_preds = a;
        let param_env = ParamEnv::new(
            tcx.mk_predicates(&synthetic_impl_preds),
            Reveal::UserFacing,
            // FIXME: Do we need to deal with `const AutoTrait`?
            rustc_hir::Constness::NotConst,
        );
        // We are potentially changing param env so delete the projection cache.
        infcx.clear_projection_cache();

        debug!("param_env: {:?}", param_env);

        let ocx = ObligationCtxt::new(&infcx);
        let mut synthetic_impl_preds_changed = false;
        ocx.register_bound(dummy_cause.clone(), param_env, self_ty, auto_trait);
        ocx.register_obligation(Obligation::new(
            tcx,
            dummy_cause.clone(),
            param_env,
            ty::Binder::dummy(ty::PredicateKind::WellFormed(self_ty.into())),
        ));

        let errors = ocx.select_where_possible();
        debug!("errors: {:?}", &errors);
        if errors.len() > 0 {
            break Err(NoSolution);
        }

        let ambiguity_errors = ocx.select_all_or_error();
        debug!("ambiguity_errors: {:?}", &ambiguity_errors);
        for err in ambiguity_errors.into_iter() {
            match err.obligation.predicate.kind().skip_binder() {
                ty::PredicateKind::Clause(clause) => match clause {
                    ty::Clause::Trait(_)
                    | ty::Clause::RegionOutlives(_)
                    | ty::Clause::TypeOutlives(_)
                    | ty::Clause::Projection(_) => {
                        let mut visitor = InferVarCollector::default();
                        let o = infcx.resolve_vars_if_possible(err.obligation);
                        o.visit_with(&mut visitor);

                        // Only add predicates to the paramenv if the infer vars contained inside
                        // are ones present in the self ty as those will get constrained to concrete
                        // types/consts by the end of this fn (unless we return `NoSolution` at
                        // which point it doesn't really matter)
                        //
                        // We don't check `allowed_infers.lts` as lifetime variables don't get` resolved
                        // until regionck which we aren't ready to do yet.
                        if visitor.tys.is_subset(&allowed_infers.tys)
                            && visitor.cts.is_subset(&allowed_infers.cts)
                        {
                            synthetic_impl_preds_changed = true;
                            synthetic_impl_preds.push(o.predicate);
                        }
                    }

                    ty::Clause::ConstArgHasType(_, _) => continue,
                },
                // non-clauses cannot be added to paramenv
                // (note: this isnt quite right but it will be eventually)
                _ => continue,
            }
        }

        if !synthetic_impl_preds_changed {
            for (param, infer) in param_to_infers.tys {
                if infcx.resolve_vars_if_possible(infer).is_ty_or_numeric_infer() {
                    let r = infcx.at(&dummy_cause, param_env).eq(param, infer);
                    assert!(r.unwrap().obligations.is_empty());
                }
            }
            for (param, infer) in param_to_infers.cts {
                if infcx.resolve_vars_if_possible(infer).is_ct_infer() {
                    let r = infcx.at(&dummy_cause, param_env).eq(param, infer);
                    assert!(r.unwrap().obligations.is_empty());
                }
            }

            // FIXME: deal with region vars

            break Ok(());
        }
    };

    let (self_ty, synthetic_impl_preds) =
        infcx.resolve_vars_if_possible((self_ty, synthetic_impl_preds));
    result.map(|()| (self_ty, tcx.mk_predicates(&synthetic_impl_preds)))
}

fn naive_self_ty_and_predicates_for_synthetic_auto_trait_impl<'tcx>(
    tcx: TyCtxt<'tcx>,
    self_ty: ty::EarlyBinder<Ty<'tcx>>,
    auto_trait: DefId,
) -> Result<(Ty<'tcx>, &'tcx List<Predicate<'tcx>>), NoSolution> {
    unreachable!()
}

#[derive(Default)]
struct InferVarCollector<'tcx> {
    tys: FxHashSet<Ty<'tcx>>,
    cts: FxHashSet<Const<'tcx>>,
    lts: FxHashSet<Region<'tcx>>,
}

impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for InferVarCollector<'tcx> {
    type BreakTy = !;

    fn visit_const(&mut self, ct: Const<'tcx>) -> ControlFlow<!> {
        if let ConstKind::Infer(_) = ct.kind() {
            self.cts.insert(ct);
        }

        ct.super_visit_with(self)
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<!> {
        if let ty::Infer(_) = ty.kind() {
            self.tys.insert(ty);
        }

        ty.super_visit_with(self)
    }

    fn visit_region(&mut self, re: Region<'tcx>) -> ControlFlow<!> {
        if let RegionKind::ReVar(_) = re.kind() {
            self.lts.insert(re);
        }

        ControlFlow::Continue(())
    }
}

#[derive(Default)]
struct InstantiatedEarlyBinderMapping<'tcx> {
    tys: FxHashMap<Ty<'tcx>, Ty<'tcx>>,
    cts: FxHashMap<Const<'tcx>, Const<'tcx>>,
    lts: FxHashMap<Region<'tcx>, Region<'tcx>>,
}

struct InstantiateEarlyBinderWithInfer<'a, 'tcx> {
    infcx: &'a InferCtxt<'tcx>,
    mapping: InstantiatedEarlyBinderMapping<'tcx>,
}

impl<'tcx> TypeFolder<TyCtxt<'tcx>> for InstantiateEarlyBinderWithInfer<'_, 'tcx> {
    fn interner(&self) -> TyCtxt<'tcx> {
        self.infcx.tcx
    }

    fn fold_const(&mut self, c: Const<'tcx>) -> Const<'tcx> {
        match c.kind() {
            ty::ConstKind::Param(_) => {
                let ty = c.ty().fold_with(self);
                *self.mapping.cts.entry(c).or_insert_with(|| {
                    self.infcx.next_const_var(
                        ty,
                        ConstVariableOrigin {
                            span: DUMMY_SP,
                            kind: ConstVariableOriginKind::MiscVariable,
                        },
                    )
                })
            }
            _ => c.super_fold_with(self),
        }
    }

    fn fold_ty(&mut self, ty: Ty<'tcx>) -> Ty<'tcx> {
        match ty.kind() {
            &ty::Param(_) => *self.mapping.tys.entry(ty).or_insert_with(|| {
                self.infcx.next_ty_var(TypeVariableOrigin {
                    span: DUMMY_SP,
                    kind: TypeVariableOriginKind::MiscVariable,
                })
            }),
            _ => ty.super_fold_with(self),
        }
    }

    fn fold_region(&mut self, r: Region<'tcx>) -> Region<'tcx> {
        match r.kind() {
            RegionKind::ReEarlyBound(_) => *self.mapping.lts.entry(r).or_insert_with(|| {
                self.infcx.next_region_var(RegionVariableOrigin::MiscVariable(DUMMY_SP))
            }),
            _ => r,
        }
    }
}
