let gcd = fn(a: i64, b: i64) -> i64 {
    if (b == 0) {
        a
    } else {
        gcd(b, a - a / b * b);
    }
};

let lcm = fn(a: i64, b: i64) -> i64 {
    a / gcd(a, b) * b;
};

print(gcd(48, 18));
print(lcm(12, 8));

let clamp = fn(value: i64, low: i64, high: i64) -> i64 {
    if (value < low) {
        low
    } else {
        if (value > high) {
            high
        } else {
            value
        }
    }
};

print(clamp(-5, 0, 100));
print(clamp(50, 0, 100));
print(clamp(200, 0, 100));

let apply_n = fn(f, n: i64, x: i64) -> i64 {
    let result = x;
    let i = 0;
    while (i < n) {
        let result = f(result);
        let i = i + 1;
    }
    result;
};

let double = fn(x: i64) -> i64 { x * 2; };
print(apply_n(double, 5, 1));

let compose = fn(f, g) {
    fn(x: i64) -> i64 { f(g(x)); };
};

let inc = fn(x: i64) -> i64 { x + 1; };
let square = fn(x: i64) -> i64 { x * x; };
let inc_then_square = compose(square, inc);
let square_then_inc = compose(inc, square);

print(inc_then_square(4));
print(square_then_inc(4));
