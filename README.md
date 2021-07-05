# All-Or-Nothing-Transform

This crate is a very early stage of development. It is intended to
implement the "Package Transform" (All-Or-Nothing-Transform) as
described by Ron L. Rivest in his paper ["Chaffing and Winnowing:
Confidentiality without
Encryption"](http://people.csail.mit.edu/rivest/chaffing-980701.txt)

Some relevant wikipedia links:

* [Chaffing and Winnowing](https://en.wikipedia.org/wiki/Chaffing_and_winnowing)
* [All-or-nothing-transform](https://en.wikipedia.org/wiki/All-or-nothing_transform)

## Currently Implemented

* encode and decode using SHA-1 on a message stored in memory


## Future Direction

- [ ] Add high-level routines to encode/decode files
- [ ] Add option to output public parameter at start of message/stream
- [ ] Symmetric option to read that during decoding
- [ ] Generic version that works with any hash routine that implements `Digest`
- [ ] Add support for different ways of combining hash parameters (currently concatenated, implement xor)
- [ ] Add support for turning encryption algorithms into digest functions (eg, AES-CBC)
- [ ] Write inner/outer (en/de)coding algorithms as traits implementing `Digest`(?)
