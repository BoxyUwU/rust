const LENGTH: f64 = 2;
//~^ ERROR mismatched types

struct Thing {
    f: [[f64; 2]; LENGTH],
}

fn main() {
    let _t = Thing { f: [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]] };
}
