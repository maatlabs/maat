let sum = 0;
for x in [10, 20, 30, 40, 50] {
    let sum = sum + x;
}
print(sum);

let n = 1;
let total = 0;
while (n < 101) {
    let total = total + n;
    let n = n + 1;
}
print(total);

let count = 0;
loop {
    let count = count + 1;
    if (count == 10) {
        break;
    }
}
print(count);

let is_prime = fn(n: i64) {
    if (n < 2) {
        return false;
    }
    let d = 2;
    while (d * d < n + 1) {
        if (n / d * d == n) {
            return false;
        }
        let d = d + 1;
    }
    true;
};

let sieve = fn(candidate: i64, limit: i64, acc) {
    if (candidate == limit) {
        acc
    } else {
        if (is_prime(candidate)) {
            sieve(candidate + 1, limit, acc.push(candidate));
        } else {
            sieve(candidate + 1, limit, acc);
        }
    }
};

let primes = sieve(2, 50, []);
print(primes);

let matrix = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
let flat = [];
for row in matrix {
    for val in row {
        let flat = flat.push(val);
    }
}
print(flat);
