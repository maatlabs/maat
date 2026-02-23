let unless = macro(cond, cons, alt) {
    quote(if (!(unquote(cond))) {
        unquote(cons)
    } else {
        unquote(alt)
    });
};

let double = macro(x) {
    quote(unquote(x) * 2);
};

print(unless(false, 42, 0));
print(double(21));
print(unless(true, 1, double(50)));
