Generic IO
==========

A type safe, low level replacement for `std::io`.

Supports `no_std` for embedded development, just disable cargo feature
`std`.

Important
---------

The development of this crate stalled for a while but there's an effort to revive and **redesign** it.
Do **not** expect the API to stay like this, the hange will be big.
Especially regarding uninitialized buffers which are **unsound** in the current version.
I'll be happy to receive [feedback on the redesign](https://github.com/Kixunil/genio/issues/23)!

Motivation
----------

The IO routines you can find in standard library are very useful. However, because they use `io::Error` as the only error type they suffer these problems:

* It's impossible to express infallible operations (e.g. in-memory operations) in types.
* If you know the operation can't fail, you still have to call `unwrap()`. If you did mistake you get runtime error.
* Compiler has to insert check (compare and branch), which slows down the code. Of course, it can be eliminated by inlining and optimization but it's not sure thing.
* `io::Error` may allocate, which makes not just slower but prevents usage on bare metal.
* `io::Error` is very broad and in some contexts it can have nonsense values (e.g. full disk when doing `read()`). Perfect program should not ignore these values but it's difficult to decide what to do about them.
* You can reasonably decide what to do with error only using `ErrorKind`. That way some information may be lost.
* The `io::Error` propagates to other interfaces and contaminates them. For example `protobuf` parser "could" return it even in case it's reading memory. Tokio-core uses `io::Error` everywhere.
* Sometimes it is hard to tell from signature what exactly can fail and why. E.g. do you know why `tokio_core::reactor::Core::new()` may fail and what to do with the error?

The aim of `genio` is to enable better exploitation of Rust's type system to solve these problems. It steals many ideas from `std::io` with one important difference: `Read` and `Write` traits can define their own errors. There's also bunch of tools for handling errors as well as glue for `std::io`.

Since everything that impls `genio::{Read, Write}` can trivially impl `std::io::{Read, Write}`, it's better to write algorithms just for `genio` and let users wrap them in glue.

Contributing
------------

This crate is certainly not finished project and it needs more work to perfectly work with `std::io`. It'd also be beneficial to implement additional algorithms, wrappers and tools. Finally, other crates should start using `genio`. I invite everyone to help with this crate, to be as good as it can be.

I'm open to every PR you can imagine.

Status
------

This crate is considered unstable, although mandatory types and methods in `Read` and `Write` traits are unlikely to change. Currently, there are only traits for **synchronous streams**. Please, if your type is not synchronous stream, don't implement these traits, but help designing other traits.

Planned:

- [X] Synchronous streams
- [ ] Asynchronous streams
- [ ] Synchronous message streams with known message size (usually in the header of message)
- [ ] Synchronous message streams with unknown size (usually using some delimiter)
- [ ] Asynchronous message streams with known message size (usually in the header of message)
- [ ] Asynchronous message streams with unknown size (usually using some delimiter)
- [ ] Sound impls for primitive types
- [ ] Sound impls for `std::io` types
- [ ] Integrate with [partial-io](https://github.com/facebookincubator/rust-partial-io)

And of course, appropriate combinators for all of them.

Differences between `genio` and `std::io`
-----------------------------------------

There are other differences than just associated error types. Most importantly, `genio` aims to implement thinner wrappers and layers. For example, it doesn't handle EINTR automatically, but requires you to handle it yourself - for example, using `Restarting` wrapper. This gives you better control of what's going on in the code and also simplifies the implementation of the wrappers.

Some operations use slightly different types. `read_exact` may return `UnexpectedEnd` in addition to lower-level error type. Chain combines two error types. `read_to_end` enables to use any type which can be extended from reader. Flushing can return different error than `write()`. If there's nothing to flush, it can return `Void` type.

In addition, `read()` may hint that there are not enough bytes (usable to skip reading in error cases) and `Write` may be hinted how many bytes will be written. (This is unstable yet.)

Blanket impls
-------------

There are intentionally no blanket impls of `genio` traits for `std::io` or vice versa. This is to enable people to implement both traits. If you use `std::io` traits in your crates, you can add support for `genio` without fear. Please, do **not** impl `genio` traits with associated types being `io::Error`. If you don't have time to write full-featured impls, stick to glue wrappers. Don't waste opportunity to implement `genio` the best way!

Disclaimer
----------

I highly appreciate all the hard work Rust developers did to create `std::io`. The `genio` crate isn't meant to bash them, judge them, etc. They likely didn't have enough time to implement something like this or feared it'd be too complicated. The aim of this crate is to improve the world by providing tool for people who want more generic IO.
