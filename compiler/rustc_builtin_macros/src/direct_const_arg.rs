use rustc_ast::tokenstream::TokenStream;
use rustc_ast::{ast, MgcaDisambiguation};
use rustc_expand::base::{self, DummyResult, ExpandResult, ExtCtxt, MacroExpanderResult};
use rustc_span::Span;

pub(crate) fn expand<'cx>(
    cx: &'cx mut ExtCtxt<'_>,
    sp: Span,
    tts: TokenStream,
) -> MacroExpanderResult<'cx> {
    let mut parser = cx.new_parser_from_tts(tts);
    let anon_const = match parser.parse_expr_anon_const(|_, _| MgcaDisambiguation::Direct) {
        Ok(parsed) => parsed,
        Err(err) => {
            return ExpandResult::Ready(DummyResult::any(sp, err.emit()));
        }
    };

    ExpandResult::Ready(Box::new(base::MacEager {
        expr: Some(Box::new(ast::Expr { 
            id: anon_const.id,
            kind: ast::ExprKind::DirectConstArg(anon_const.value.clone()),
            span: anon_const.value.span,
            attrs: Default::default(),
            tokens: None, 
        })),
        ty: Some(Box::new(ast::Ty {
            id: anon_const.id,
            kind: ast::TyKind::DirectConstArg(anon_const.value.clone()),
            span: anon_const.value.span,
            tokens: None,
        })),
        ..Default::default()
    }))
}