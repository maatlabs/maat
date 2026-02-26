let factorial = fn(n: i64) -> i64 {
    if (n == 0) {
        1
    } else {
        n * factorial(n - 1);
    }
};

let factorial_iter = fn(n: i64) -> i64 {
    let result = 1;
    let i = 1;
    while (i < n + 1) {
        let result = result * i;
        let i = i + 1;
    }
    result;
};

print(factorial(10));
print(factorial_iter(10));
print(factorial(10) == factorial_iter(10));
