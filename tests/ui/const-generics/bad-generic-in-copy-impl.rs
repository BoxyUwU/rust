#[derive(Copy, Clone)]
//~^ ERROR the trait `Copy` cannot be implemented for this type
pub struct Foo {
    x: [u8; SIZE],
    //~^ ERROR the constant `1` is not of type `usize`
    //~| ERROR the constant `1` is not of type `usize`
}

const SIZE: u32 = 1;

fn main() {}
