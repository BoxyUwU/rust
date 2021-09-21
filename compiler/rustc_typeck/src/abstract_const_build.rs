use rustc_errors::ErrorReported;
use rustc_hir::{self as hir, def::DefKind};
use rustc_middle::thir;
use rustc_middle::ty::{self, DefIdTree, TyCtxt, TypeFoldable};
use rustc_span::def_id::LocalDefId;

/// Builds an abstract const, do not use this directly, but use `AbstractConst::new` instead.
pub(super) fn thir_abstract_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: ty::WithOptConstParam<LocalDefId>,
) -> Result<Option<&'tcx [thir::abstract_const::Node<'tcx>]>, ErrorReported> {
    if tcx.lazy_normalization() == false {
        return Ok(None);
    }

    match tcx.def_kind(def.did) {
        // FIXME(generic_const_exprs): We currently only do this for anonymous constants,
        // meaning that we do not look into associated constants. I(@lcnr) am not yet sure whether
        // we want to look into them or treat them as opaque projections.
        //
        // Right now we do neither of that and simply always fail to unify them.
        DefKind::AnonConst => (),
        _ => return Ok(None),
    }
    debug!("thir_abstract_const: def={:?}", def.did);

    let anon_ct_hir_id = tcx.hir().local_def_id_to_hir_id(def.did);
    match tcx.hir().get(anon_ct_hir_id) {
        hir::Node::AnonConst(anon_ct) => {
            tcx.hir().is_a_fully_qualified_associated_const_expr(anon_ct.body)
        }
        _ => None,
    }
    .map(|(this, path)| {
        let item_did = tcx.parent(path.res.def_id()).unwrap();
        debug!("item_did: {:?}", item_did);
        let item_ctxt =
            &crate::collect::ItemCtxt::new(tcx, item_did) as &dyn crate::astconv::AstConv<'_>;
        let self_ty = item_ctxt.ast_ty_to_ty(this);
        debug!("self_ty: {:?}", self_ty);
        // FIXME(type_level_assoc_const): this code is definitely wrong for
        // resolve QPaths that aren't `<T as Trait<..>>::ASSOC`
        let trait_ref = <dyn crate::astconv::AstConv<'_>>::ast_path_to_mono_trait_ref(
            item_ctxt,
            rustc_span::DUMMY_SP,
            item_did,
            self_ty,
            &path.segments[0],
        );
        debug!("trait_ref_substs: {:?}", trait_ref.substs);
        let assoc_item_substs =
            <dyn crate::astconv::AstConv<'_>>::create_substs_for_associated_item(
                item_ctxt,
                tcx,
                rustc_span::DUMMY_SP,
                path.res.def_id(),
                &path.segments[1],
                trait_ref.substs,
            );
        debug!("assoc_item_substs: {:?}", assoc_item_substs);

        // FIXME(type_level_assoc_const): this is probably not the right way to
        // determine whether the projection is concrete or not.
        use rustc_middle::thir::abstract_const::Node;
        let ct = tcx.mk_const(ty::Const {
            val: ty::ConstKind::Unevaluated(ty::Unevaluated::new(
                ty::WithOptConstParam::unknown(path.res.def_id()),
                assoc_item_substs,
            )),
            ty: tcx.type_of(path.res.def_id()),
        });
        Ok(assoc_item_substs
            .definitely_has_param_types_or_consts(tcx)
            .then(|| &*tcx.arena.alloc_from_iter([Node::Leaf(ct)])))
    })
    .unwrap_or_else(|| {
        let body = tcx.thir_body(def);
        if body.0.borrow().exprs.is_empty() {
            // type error in constant, there is no thir
            return Err(ErrorReported);
        }

        use rustc_trait_selection::traits::const_evaluatable::AbstractConstBuilder;
        AbstractConstBuilder::new(tcx, (&*body.0.borrow(), body.1))?
            .map(AbstractConstBuilder::build)
            .transpose()
    })
}
