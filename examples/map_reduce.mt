let map = fn(arr, f) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            iter(rest(arr), push(acc, f(first(arr))));
        }
    };
    iter(arr, []);
};

let reduce = fn(arr, init, f) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            iter(rest(arr), f(acc, first(arr)));
        }
    };
    iter(arr, init);
};

let numbers = [1, 2, 3, 4, 5];
let double = fn(x) { x * 2 };
let add = fn(a, b) { a + b };

let doubled = map(numbers, double);
let sum = reduce(doubled, 0, add);

print(doubled);
print(sum);
