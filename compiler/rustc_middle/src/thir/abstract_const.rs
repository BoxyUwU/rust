//! A subset of a mir body used for const evaluatability checking.
use crate::mir;
use crate::ty::subst::SubstsRef;
use crate::ty::{self, AdtDef, Ty, TyCtxt};
use rustc_errors::ErrorReported;
use rustc_target::abi::VariantIdx;

rustc_index::newtype_index! {
    /// An index into an `AbstractConst`.
    pub struct NodeId {
        derive [HashStable]
        DEBUG_FORMAT = "n{}",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, HashStable, TyEncodable, TyDecodable)]
pub enum CastKind {
    /// thir::ExprKind::As
    As,
    /// thir::ExprKind::Use
    Use,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, HashStable, TyEncodable, TyDecodable)]
pub struct ACFieldExpr {
    pub name: mir::Field,
    pub expr: NodeId,
}

/// A node of an `AbstractConst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, HashStable, TyEncodable, TyDecodable)]
pub enum Node<'tcx> {
    Leaf(&'tcx ty::Const<'tcx>),
    Binop(mir::BinOp, NodeId, NodeId),
    UnaryOp(mir::UnOp, NodeId),
    FunctionCall(NodeId, &'tcx [NodeId]),
    Cast(CastKind, NodeId, Ty<'tcx>),
    Adt(&'tcx AdtDef, VariantIdx, SubstsRef<'tcx>, &'tcx [ACFieldExpr]),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, HashStable, TyEncodable, TyDecodable)]
pub enum NotConstEvaluatable {
    Error(ErrorReported),
    MentionsInfer,
    MentionsParam,
}

impl From<ErrorReported> for NotConstEvaluatable {
    fn from(e: ErrorReported) -> NotConstEvaluatable {
        NotConstEvaluatable::Error(e)
    }
}

TrivialTypeFoldableAndLiftImpls! {
    NotConstEvaluatable,
}

impl<'tcx> TyCtxt<'tcx> {
    #[inline]
    pub fn thir_abstract_const_opt_const_arg(
        self,
        def: ty::WithOptConstParam<rustc_hir::def_id::DefId>,
    ) -> Result<Option<&'tcx [Node<'tcx>]>, ErrorReported> {
        if let Some((did, param_did)) = def.as_const_arg() {
            self.thir_abstract_const_of_const_arg((did, param_did))
        } else {
            self.thir_abstract_const(def.did)
        }
    }
}
