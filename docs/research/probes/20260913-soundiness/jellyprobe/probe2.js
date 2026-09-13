const handlers = { a: () => 1, b: () => 2 };
function dispatch(k) { return handlers[k](); }
dispatch("a");
