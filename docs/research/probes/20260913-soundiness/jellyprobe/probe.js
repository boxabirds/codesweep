// A: plain call, should resolve
function save(x) { return x; }
save(1);

// B: dispatch through a table built from a computed key -- hard
const handlers = { a: () => 1, b: () => 2 };
function dispatch(k) { return handlers[k](); }
dispatch(process.argv[2]);

// C: call through a value that came from unmodelled native code
const fn = require('os').platform;
fn();

// D: call a method on an object returned by eval
const o = eval("({ run: function(){ return 3; } })");
o.run();
