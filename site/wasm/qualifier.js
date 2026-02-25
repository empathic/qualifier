// Qualifier WASM glue — placeholder
// Replace with Emscripten-generated glue when building from Rust source.
//
// This stub exports a createModule() that returns a promise resolving to
// a minimal Module object.  The real build will use:
//   cargo build --target wasm32-unknown-emscripten --release
//   (or wasm-pack / wasm-bindgen, depending on strategy)

var createQualifierModule = (function () {
  "use strict";

  return function createModule(opts) {
    opts = opts || {};
    return Promise.resolve({
      _placeholder: true,
      callMain: function (args) {
        // The playground's mock layer handles all output before this is
        // ever called, so this is truly just a structural placeholder.
        return 0;
      },
    });
  };
})();

if (typeof module !== "undefined") module.exports = createQualifierModule;
