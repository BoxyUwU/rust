//@ compile-flags: -Znext-solver
// check-pass

trait Bound<'a> {}
trait Trait<'a> {
    type Assoc: Bound<'a>;
}

fn foo<'a, T>()
where
    T: Trait<'a>,
    <T as Trait<'a>>::Assoc: Bound<'a>,
{
}

fn main() {}
