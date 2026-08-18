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

fn foo<T>()
where
    T: Trait,
    for<'a> T::Assoc<'a>: Send,
{}

fn bar<'b>() {
    foo::<&'b ()>()
}

trait Intermediate {}
impl<T: Trait> Intermediate for T
where
    for<'a> T::Assoc<'a>: Send, {}

fn foo2<T>()
where
    T: Intermediate,
{}

fn bar2<'b>() {
    // foo2::<&'b ()>()
}

fn main() {}