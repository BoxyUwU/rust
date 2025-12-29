use rustc_abi::Size;
use rustc_ast::{self as ast};
use rustc_hir::LangItem;
use rustc_middle::bug;
use rustc_middle::mir::interpret::LitToConstInput;
use rustc_middle::ty::{self, Ty, ScalarInt, TyCtxt, TypeVisitableExt as _};
use tracing::trace;

use crate::builder::parse_float_into_scalar;

pub(crate) fn lit_to_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    lit_input: LitToConstInput<'tcx>,
) -> ty::Const<'tcx> {
    let LitToConstInput { lit, ty, neg } = lit_input;

    if let Err(guar) = ty.error_reported() {
        return ty::Const::new_error(tcx, guar);
    }

    let trunc = |n, width: ty::UintTy| {
        let width = width
            .normalize(tcx.data_layout.pointer_size().bits().try_into().unwrap())
            .bit_width()
            .unwrap();
        let width = Size::from_bits(width);
        trace!("trunc {} with size {} and shift {}", n, width.bits(), 128 - width.bits());
        let result = width.truncate(n);
        trace!("trunc result: {}", result);

        ScalarInt::try_from_uint(result, width)
            .unwrap_or_else(|| bug!("expected to create ScalarInt from uint {:?}", result))
    };

    let val = match (lit, ty.kind()) {
        (ast::LitKind::Str(s, _), ty::Ref(_, inner_ty, _)) if inner_ty.is_str() => {
            let str_bytes = s.as_str().as_bytes();
            ty::Value::from_raw_bytes(tcx, str_bytes, ty)
        }
        (ast::LitKind::Str(s, _), ty::Str) if tcx.features().deref_patterns() => {
            // String literal patterns may have type `str` if `deref_patterns` is enabled, in order
            // to allow `deref!("..."): String`.
            let str_bytes = s.as_str().as_bytes();
            ty::Value::from_raw_bytes(tcx, str_bytes, ty)
        }
        (ast::LitKind::ByteStr(byte_sym, _), ty::Ref(_, inner_ty, _))
            if matches!(inner_ty.kind(), ty::Slice(_) | ty::Array(..)) =>
        {
            ty::Value::from_raw_bytes(tcx, byte_sym.as_byte_str(), ty)
        }
        (ast::LitKind::ByteStr(byte_sym, _), ty::Slice(_) | ty::Array(..))
            if tcx.features().deref_patterns() =>
        {
            // Byte string literal patterns may have type `[u8]` or `[u8; N]` if `deref_patterns` is
            // enabled, in order to allow, e.g., `deref!(b"..."): Vec<u8>`.
            ty::Value::from_raw_bytes(tcx, byte_sym.as_byte_str(), ty)
        }
        (ast::LitKind::Byte(n), ty::Uint(ty::UintTy::U8)) => {
            ty::Value::from_scalar_int(tcx, n.into(), ty)
        }
        (ast::LitKind::CStr(byte_sym, _), ty::Ref(_, inner_ty, _)) if matches!(inner_ty.kind(), ty::Adt(def, _) if tcx.is_lang_item(def.did(), LangItem::CStr)) =>
        {
            // A CStr is a newtype around a byte slice, so we create the inner slice here.
            // We need a branch for each "level" of the data structure.
            let bytes_ty = Ty::new_slice(tcx, tcx.types.u8);
            let bytes = ty::Value::from_raw_bytes(tcx, byte_sym.as_byte_str(), bytes_ty);
            ty::Value::from_branches(tcx, [ty::Const::new_value_direct(tcx, bytes)], ty)
        }
        (ast::LitKind::Int(n, _), ty::Uint(ui)) if !neg => {
            let scalar_int = trunc(n.get(), *ui);
            ty::Value::from_scalar_int(tcx, scalar_int, ty)
        }
        (ast::LitKind::Int(n, _), ty::Int(i)) => {
            let scalar_int = trunc(
                if neg { (n.get() as i128).overflowing_neg().0 as u128 } else { n.get() },
                i.to_unsigned(),
            );
            ty::Value::from_scalar_int(tcx, scalar_int, ty)
        }
        (ast::LitKind::Bool(b), ty::Bool) => ty::Value::from_scalar_int(tcx, b.into(), ty),
        (ast::LitKind::Float(n, _), ty::Float(fty)) => {
            let bits = parse_float_into_scalar(n, *fty, neg).unwrap_or_else(|| {
                tcx.dcx().bug(format!("couldn't parse float literal: {:?}", lit_input.lit))
            });
            ty::Value::from_scalar_int(tcx, bits, ty)
        }
        (ast::LitKind::Char(c), ty::Char) => ty::Value::from_scalar_int(tcx, c.into(), ty),
        (ast::LitKind::Err(guar), _) => return ty::Const::new_error(tcx, guar),
        _ => return ty::Const::new_misc_error(tcx),
    };

    ty::Const::new_value_direct(tcx, val)
}
