// build-pass

pub struct Wrapper(for<'b> fn(&'b ()));

fn useful(_: &()) {}

fn main() {
    let wrapper = Wrapper(useful);
    let dyn_debug = &wrapper.0 as &fn(&'static ()) as &dyn std::fmt::Debug;
    println!("{:?}", dyn_debug);
}
