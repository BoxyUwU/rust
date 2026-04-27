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
