let map = fn(arr, f) {
    let iter = fn(arr, acc) {
        if (arr.len() == 0usize) {
            acc
        } else {
            iter(arr.rest(), acc.push(f(arr.first())));
        }
    };
    iter(arr, []);
};

let reduce = fn(arr, init, f) {
    let iter = fn(arr, acc) {
        if (arr.len() == 0usize) {
            acc
        } else {
            iter(arr.rest(), f(acc, arr.first()));
        }
    };
    iter(arr, init);
};

let filter = fn(arr, predicate) {
    let iter = fn(arr, acc) {
        if (arr.len() == 0usize) {
            acc
        } else {
            let head = arr.first();
            let tail = arr.rest();
            if (predicate(head)) {
                iter(tail, acc.push(head));
            } else {
                iter(tail, acc);
            }
        }
    };
    iter(arr, []);
};

let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let double = fn(x: i64) -> i64 { x * 2; };
let add = fn(a: i64, b: i64) -> i64 { a + b; };
let is_even = fn(x) { x / 2 * 2 == x; };

let doubled = map(numbers, double);
let evens = filter(numbers, is_even);
let sum = reduce(doubled, 0, add);

print(doubled);
print(evens);
print(sum);
