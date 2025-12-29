use std::fmt;
use std::ops::Deref;

use rustc_macros::{
    HashStable, Lift, TyDecodable, TyEncodable, TypeFoldable, TypeVisitable, extension,
};

use rustc_hir::def::Namespace;

use super::ScalarInt;
use crate::mir::interpret::{ErrorHandled, Scalar};
use crate::ty::print::{FmtPrinter, PrettyPrinter};
use crate::ty::{self, Interned, Ty, List, TyCtxt, ValTreeKind, ValTreeRecur};

use rustc_type_ir::inherent::*;

#[extension(pub trait ValTreeKindExt<'tcx>)]
impl<'tcx> FullValTreeKind<'tcx> {
    fn try_to_scalar(&self) -> Option<Scalar> {
        self.try_to_leaf().map(Scalar::Int)
    }
}

#[extension(pub trait PartialValTreeKindExt<'tcx>)]
impl<'tcx> PartialValTreeKind<'tcx> {
    fn try_to_scalar(&self) -> Option<Scalar> {
        self.try_to_leaf().map(Scalar::Int)
    }
}

/// An interned valtree. Use this rather than `ValTreeKind`, whenever possible.
///
/// See the docs of [`ty::ValTreeKind`] or the [dev guide] for an explanation of this type.
///
/// [dev guide]: https://rustc-dev-guide.rust-lang.org/mir/index.html#valtrees
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[derive(HashStable)]
pub struct ValTree<'tcx>(pub(crate) Interned<'tcx, FullValTreeKind<'tcx>>);

// FIXME: interning this list seems suboptimal but slices arent typefoldable
pub type FullValTreeKind<'tcx> = ty::ValTreeKind<TyCtxt<'tcx>, &'tcx List<ValTree<'tcx>>>;
impl<'tcx> ValTreeRecur<TyCtxt<'tcx>> for &'tcx List<ValTree<'tcx>> {}

impl<'tcx> ValTree<'tcx> {
    /// Returns the zero-sized valtree: `Branch([])`.
    pub fn zst(tcx: TyCtxt<'tcx>) -> Self {
        tcx.consts.valtree_zst
    }

    pub fn from_raw_bytes(tcx: TyCtxt<'tcx>, bytes: &[u8]) -> Self {
        let branches = bytes.iter().map(|&b| {
            Self::from_scalar_int(tcx, b.into())
        });
        Self::from_branches(tcx, branches)
    }

    pub fn from_branches(
        tcx: TyCtxt<'tcx>,
        branches: impl IntoIterator<Item = ty::ValTree<'tcx>>,
    ) -> Self {
        let kind = ty::ValTreeKind::Branch(&*tcx.mk_valtree_list_from_iter(branches.into_iter()));
        tcx.intern_full_valtree(kind)
    }

    pub fn from_scalar_int(tcx: TyCtxt<'tcx>, i: ScalarInt) -> Self {
        let kind = ty::ValTreeKind::Leaf(i);
        tcx.intern_full_valtree(kind)
    }
}

impl<'tcx> Deref for ValTree<'tcx> {
    type Target = &'tcx FullValTreeKind<'tcx>;

    #[inline]
    fn deref(&self) -> &&'tcx FullValTreeKind<'tcx> {
        &self.0.0
    }
}

impl fmt::Debug for ValTree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// `Ok(Err(ty))` indicates the constant was fine, but the valtree couldn't be constructed
/// because the value contains something of type `ty` that is not valtree-compatible.
/// The caller can then show an appropriate error; the query does not have the
/// necessary context to give good user-facing errors for this case.
pub type ConstToValTreeResult<'tcx> = Result<Result<ValTree<'tcx>, Ty<'tcx>>, ErrorHandled>;

/// A type-level constant value.
///
/// Represents a typed, fully evaluated constant.
/// Note that this is also used by pattern elaboration to represent values which cannot occur in types,
/// such as raw pointers and floats.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[derive(HashStable, TyEncodable, TyDecodable, TypeFoldable, TypeVisitable, Lift)]
pub struct Value<'tcx> {
    pub ty: Ty<'tcx>,
    pub valtree: PartialValTree<'tcx>,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[derive(HashStable)]
pub struct PartialValTree<'tcx>(pub(crate) Interned<'tcx, PartialValTreeKind<'tcx>>);

// FIXME: interning this list seems suboptimal but its not `TypeFoldable` otherwise lol
pub type PartialValTreeKind<'tcx> = ty::ValTreeKind<TyCtxt<'tcx>, &'tcx List<ty::Const<'tcx>>>;
impl<'tcx> ValTreeRecur<TyCtxt<'tcx>> for &'tcx List<ty::Const<'tcx>> {}

impl<'tcx> IntoKind for PartialValTree<'tcx> {
    type Kind = PartialValTreeKind<'tcx>;
    fn kind(self) -> PartialValTreeKind<'tcx> {
        *self.0
    }
}

impl<'tcx> Deref for PartialValTree<'tcx> {
    type Target = &'tcx PartialValTreeKind<'tcx>;

    #[inline]
    fn deref(&self) -> &&'tcx PartialValTreeKind<'tcx> {
        &self.0.0
    }
}

impl fmt::Debug for PartialValTree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}


impl<'tcx> Value<'tcx> {
    pub fn to_full_valtree(&self) -> ValTree<'tcx> {
        todo!()
    }
    
    /// Returns the zero-sized valtree: `Branch([])`.
    pub fn zst(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Self {
        Self::from_branches(tcx, [], ty)
    }
    
    pub fn from_raw_bytes(tcx: TyCtxt<'tcx>, bytes: &[u8], ty: Ty<'tcx>) -> Self {
        let branches = bytes.iter().map(|&b| {
            ty::Const::new_value_direct(tcx, Self::from_scalar_int(tcx, b.into(), tcx.types.u8))
        });
        Self::from_branches(tcx, branches, todo!())
    }

    pub fn from_branches(
        tcx: TyCtxt<'tcx>,
        branches: impl IntoIterator<Item = ty::Const<'tcx>>,
        ty: Ty<'tcx>,
    ) -> Self {
        let branches = tcx.mk_const_list_from_iter(branches.into_iter());
        let valtree = tcx.intern_partial_valtree(ty::ValTreeKind::Branch(branches));
        ty::Value {
            ty,
            valtree,
        }
    }

    pub fn from_scalar_int(tcx: TyCtxt<'tcx>, i: ScalarInt, ty: Ty<'tcx>) -> Self {
        let valtree = tcx.intern_partial_valtree(ty::ValTreeKind::Leaf(i));
        ty::Value {
            ty,
            valtree,
        }
    }
    
    /// Attempts to extract the raw bits from the constant.
    ///
    /// Fails if the value can't be represented as bits (e.g. because it is a reference
    /// or an aggregate).
    #[inline]
    pub fn try_to_bits(self, tcx: TyCtxt<'tcx>, typing_env: ty::TypingEnv<'tcx>) -> Option<u128> {
        let (ty::Bool | ty::Char | ty::Uint(_) | ty::Int(_) | ty::Float(_)) = self.ty.kind() else {
            return None;
        };
        let scalar = self.try_to_leaf()?;
        let input = typing_env.with_post_analysis_normalized(tcx).as_query_input(self.ty);
        let size = tcx.layout_of(input).ok()?.size;
        Some(scalar.to_bits(size))
    }

    pub fn try_to_bool(self) -> Option<bool> {
        if !self.ty.is_bool() {
            return None;
        }
        self.try_to_leaf()?.try_to_bool().ok()
    }

    pub fn try_to_target_usize(self, tcx: TyCtxt<'tcx>) -> Option<u64> {
        if !self.ty.is_usize() {
            return None;
        }
        self.try_to_leaf().map(|s| s.to_target_usize(tcx))
    }

    /// Get the values inside the ValTree as a slice of bytes. This only works for
    /// constants with types &str, &[u8], or [u8; _].
    pub fn try_to_raw_bytes(self, tcx: TyCtxt<'tcx>) -> Option<&'tcx [u8]> {
        match self.ty.kind() {
            ty::Ref(_, inner_ty, _) => match inner_ty.kind() {
                // `&str` can be interpreted as raw bytes
                ty::Str => {}
                // `&[u8]` can be interpreted as raw bytes
                ty::Slice(slice_ty) if *slice_ty == tcx.types.u8 => {}
                // other `&_` can't be interpreted as raw bytes
                _ => return None,
            },
            // `[u8; N]` can be interpreted as raw bytes
            ty::Array(array_ty, _) if *array_ty == tcx.types.u8 => {}
            // Otherwise, type cannot be interpreted as raw bytes
            _ => return None,
        }

        Some(tcx.arena.alloc_from_iter(self.to_branch().into_iter().map(|ct| ct.to_leaf().to_u8())))
    }

    /// Converts to a `ValTreeKind::Leaf` value, `panic`'ing
    /// if this constant is some other kind.
    #[inline]
    pub fn to_leaf(self) -> ScalarInt {
        match self.valtree.kind() {
            ValTreeKind::Leaf(s) => s,
            ValTreeKind::Branch(..) => bug!("expected leaf, got {:?}", self),
        }
    }

    /// Converts to a `ValTreeKind::Branch` value, `panic`'ing
    /// if this constant is some other kind.
    #[inline]
    pub fn to_branch(self) -> &'tcx [ty::Const<'tcx>] {
        match self.valtree.kind() {
            ValTreeKind::Branch(branch) => &**branch,
            ValTreeKind::Leaf(..) => bug!("expected branch, got {:?}", self),
        }
    }

    /// Attempts to convert to a `ValTreeKind::Leaf` value.
    pub fn try_to_leaf(self) -> Option<ScalarInt> {
        match self.valtree.kind() {
            ValTreeKind::Leaf(s) => Some(s),
            ValTreeKind::Branch(_) => None,
        }
    }

    /// Attempts to convert to a `ValTreeKind::Leaf` value.
    pub fn try_to_scalar(&self) -> Option<Scalar> {
        self.try_to_leaf().map(Scalar::Int)
    }

    /// Attempts to convert to a `ValTreeKind::Branch` value.
    pub fn try_to_branch(self) -> Option<&'tcx [ty::Const<'tcx>]> {
        match self.valtree.kind() {
            ValTreeKind::Branch(branch) => Some(&**branch),
            ValTreeKind::Leaf(_) => None,
        }
    }
}

impl<'tcx> rustc_type_ir::inherent::ValueConst<TyCtxt<'tcx>> for Value<'tcx> {
    fn ty(self) -> Ty<'tcx> {
        self.ty
    }

    fn valtree(self) -> PartialValTree<'tcx> {
        self.valtree
    }
}

impl<'tcx> fmt::Display for Value<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ty::tls::with(move |tcx| {
            let cv = tcx.lift(*self).unwrap();
            let mut p = FmtPrinter::new(tcx, Namespace::ValueNS);
            p.pretty_print_const_valtree(cv, /*print_ty*/ true)?;
            f.write_str(&p.into_buffer())
        })
    }
}
