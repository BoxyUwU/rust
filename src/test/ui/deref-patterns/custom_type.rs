// run-pass
#![feature(deref_patterns)]

struct Foo();
impl std::ops::Deref for Foo {
    type Target = u32;
    fn deref(&self) -> &u32 {
        &10
    }
}

fn main() {
    match Foo() {
        12 => (),
        _ => todo!(),
    }
}
