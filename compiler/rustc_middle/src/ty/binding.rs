use super::Mutability;
use rustc_hir::{BindingAnnotation, ByRef};

#[derive(Clone, PartialEq, TyEncodable, TyDecodable, Debug, Copy, HashStable)]
pub enum BindingMode {
    BindByReference(Mutability),
    BindByValue(Mutability),
}

TrivialTypeTraversalAndLiftImpls! { BindingMode, }

impl BindingMode {
    pub fn convert(BindingAnnotation(by_ref, mutbl): BindingAnnotation) -> BindingMode {
        let mutbl = match mutbl {
            rustc_ast::Mutability::Not => Mutability::Not,
            rustc_ast::Mutability::Mut => Mutability::Mut,
        };

        match by_ref {
            ByRef::No => BindingMode::BindByValue(mutbl),
            ByRef::Yes => BindingMode::BindByReference(mutbl),
        }
    }
}
