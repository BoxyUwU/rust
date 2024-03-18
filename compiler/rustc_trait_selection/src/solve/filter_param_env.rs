use crate::traits::StructurallyNormalizeExt;
use rustc_infer::{
    infer::TyCtxtInferExt,
    traits::{FulfillmentError, ObligationCause},
};
use rustc_middle::ty::{self, ClauseKind, ToPredicate, Ty, TyCtxt, TyKind};

use crate::traits::ObligationCtxt;

pub fn filter_explicit_alias_bounds<'tcx>(
    tcx: TyCtxt<'tcx>,
    env: ty::ParamEnv<'tcx>,
) -> ty::ParamEnv<'tcx> {
    let infcx = tcx.infer_ctxt().build();
    let mut ocx = ObligationCtxt::new(&infcx);

    for clause in env.caller_bounds().iter().map(|clause| clause.kind()) {
        let whatever = infcx.enter_forall(clause, |clause| {
            if let ClauseKind::Trait(pred) = clause {
                if let ty::Alias(_, _) = pred.self_ty().kind() {
                    let normalized = ocx.structurally_normalize_ty(
                        &ObligationCause::dummy(),
                        env,
                        pred.self_ty(),
                    )?;

                    if let ty::Alias(_, alias_ty) = normalized.kind() {
                        let alias_bounds =
                            tcx.item_bounds(alias_ty.def_id).instantiate(tcx, alias_ty.args);
                    } else {
                    }

                    todo!()
                }
            }

            Err::<Ty<'tcx>, _>(Vec::<FulfillmentError<'tcx>>::new())
        });
    }

    todo!()
}
