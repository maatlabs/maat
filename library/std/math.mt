pub fn abs(x: i64) -> i64 {
    if (x < 0) { -x } else { x }
}

pub fn min(a: i64, b: i64) -> i64 {
    if (a < b) { a } else { b }
}

pub fn max(a: i64, b: i64) -> i64 {
    if (a > b) { a } else { b }
}

fn pow_helper(acc: i64, b: i64, e: i64) -> i64 {
    if (e == 0) {
        acc
    } else {
        let rem: i64 = e - (e / 2) * 2;
        if (rem == 1) {
            pow_helper(acc * b, b * b, e / 2)
        } else {
            pow_helper(acc, b * b, e / 2)
        }
    }
}

pub fn pow(base: i64, exp: i64) -> i64 {
    if (exp < 0) {
        0
    } else {
        pow_helper(1, base, exp)
    }
}

fn gcd_helper(x: i64, y: i64) -> i64 {
    if (y == 0) {
        x
    } else {
        gcd_helper(y, x - (x / y) * y)
    }
}

pub fn gcd(a: i64, b: i64) -> i64 {
    gcd_helper(abs(a), abs(b))
}

pub fn lcm(a: i64, b: i64) -> i64 {
    if (a == 0) {
        0
    } else {
        if (b == 0) {
            0
        } else {
            abs(a / gcd(a, b) * b)
        }
    }
}
