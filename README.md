# memoized kerney

This crate implements a memoized version of the kernel inverse geodesic problem.

The advance is "small" but it exists. In a low contention environment looking up a value
is around ~500ns (on my local machine) while calculating the inverse result is ~850-900ns.

In higher contention scenarios this will likely mean that any caching is not worth it.
