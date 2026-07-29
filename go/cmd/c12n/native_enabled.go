//go:build c12n_native

package main

// nativeBuild reports whether this binary was compiled with the native
// c12n-core engine linked in (-tags c12n_native).
const nativeBuild = true
