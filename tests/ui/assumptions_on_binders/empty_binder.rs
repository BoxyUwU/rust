//@ compile-flags: -Znext-solver -Zassumptions-on-binders
//@ edition: 2024
//@ check-pass

// We would previously insert new assumptions into an exisitng universe
// when entering an empty binder. This caused us to incorreclty require
// `'b: 'static` instead of using a `'b: '!a` assumption.

trait Trait {
    type Assoc<'a>
    where
        Self: 'a;
}

impl<T> Trait for T {
    type Assoc<'a> = ()
    where
        Self: 'a;
}

struct Foo<T>
where
    T: Trait,
    for<'a> T::Assoc<'a>: Send,
{
    field: T,
}

struct Bar<'b>(Foo<&'b ()>);

fn main() {}
