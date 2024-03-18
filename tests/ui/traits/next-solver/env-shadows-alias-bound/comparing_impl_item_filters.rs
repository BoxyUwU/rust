//@ compile-flags: -Znext-solver
// check-pass
// repro for rust-lang/trait-system-refactor-initiative#90

trait Trait<'a, T> {
    fn foo(_: T)
    where
        T: HasSupertrait<'a>;
}

impl<'a, U: WithAssoc<'a>> Trait<'a, <U as WithAssoc<'a>>::Assoc> for U {
    fn foo(u: U::Assoc)
    where
        // In theory even if we filter out the `Bound<'a>` when constructing `foo`'s env
        // `compute_predicate_entailment` should create an artificial env with the `Bound<'a>`
        // present. So this should in theory test that we handle filtering out such bounds there too.
        //
        // The user also cannot just remove this bound as it would cause `assert_impls_trait` to fail.
        <U as WithAssoc<'a>>::Assoc: HasSupertrait<'a>,
    {
        assert_impls_trait(u);
    }
}

fn assert_impls_trait<'a>(_: impl HasSupertrait<'a>) {}

trait WithAssoc<'a> {
    type Assoc: Bound<'a>;
}

trait HasSupertrait<'a>: Bound<'a> {}
trait Bound<'a> {}

fn main() {}
