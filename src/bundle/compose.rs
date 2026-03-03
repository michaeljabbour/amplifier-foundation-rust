// Bundle composition is implemented as `Bundle::compose()` in bundle/mod.rs.
// This module exists for future decomposition if compose logic grows.
// The 5-strategy composition system (deep merge, merge by module ID,
// dict update, accumulate with namespace, later replaces) is documented
// in the architecture spec section 7.
