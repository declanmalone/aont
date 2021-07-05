//! # Rivest's All-or-nothing-transform
//!
//! ```rust
//! use aont::{encode_sha1, decode_sha1};
//!
//! // Set up message to be encoded and public parameter
//! let message = "0123456789abcdef0123";
//! let public  = "abcdeabcdeabcdeabcde";
//! let message_as_bytes  = message.as_bytes();
//!
//! // Do the encoding (get back transformed data)
//! let encoded = encode_sha1(message_as_bytes, public.as_bytes());
//! assert_ne!(*message_as_bytes, *encoded);
//!
//! // Pass encoded message and same public parameter to recover original message
//! let recover = decode_sha1(&*encoded, public.as_bytes());
//! assert_eq!(message_as_bytes, &*recover);
//!
//! ```
//!
//! # Operation
//!
//! The [AONT](https://en.wikipedia.org/wiki/All-or-nothing_transform)
//! encodes and decodes a message by use of a public "key" `P` (in the
//! normal sense of the word&mdash;not an RSA public key) and a random
//! "key" `R`. It also uses a hash function E() which works on blocks
//! of the message.
//!
//! The steps taken are:
//!
//! 1. outer encoding
//! - Apply random key `R` to each block `i` from `1..n` by XORing them with `E(R,i)`
//! - These transformed blocks will be sent to the recipient
//! 2. inner encoding
//! - For all blocks `1..n`, calculate `S` as the XOR sum of `E(P, transformed_block(i) + i)`
//! - Append block(n + 1) as S xor R
//! - (Optionally, header or trailer information can include P and/or the amount of padding)
//! 3. inner decoding
//! - For all blocks `1..n`, calculate `S` as the XOR sum of `E(P, received_block(i) + i)`
//! - XOR S with received_block(n+1) giving R
//! 4. outer decoding
//! - Apply recovered random key `R` to each recieved block `i` from
//!   `1..n` by XORing them with `E(R,i)`
//!
//! Note that this is not encryption in the usual sense. If all blocks
//! of the transformed message have been received and are placed in
//! the right order, the outer decoding step can use the public "key"
//! to extract the random "key" from the final block of the
//! message. With this, the outer decoding can proceed to decode the
//! transformed blocks.
//!
//! # Explanation in terms of XOR masking
//!
//! In simpler terms, encoding can be understood as:
//!
//! * applying an xor mask to the file using a random parameter `R`
//! * calculating a hash `S` of the transformed data using a public parameter `P`
//! * using that hash to xor mask the value of `R`, which is appended to the file
//!
//! In reverse:
//!
//! * the same public hash function is applied to all non-final blocks, recovering S
//! * the final block is XOR-ed with S to recover R
//! * The xor mask generated by R is applied to the non-final blocks
//!   to recover the original
//!
//! # "Encryption" Functions: Hash Functions
//!
//! The simplest kind of "Encryption" function for this scheme is a
//! standard one-way hash function such as SHA or MD5. In this case, a
//! "block" is the same size as the output of the hash function.
//!
//! Where the algorithm calls for passing in a "key" parameter, a
//! block, or an integer, these can be handled by concatenation. For
//! example `E, P, block[i] + i` could be implemented by string
//! concatenation of:
//!
//! * the `P` parameter in binary form
//! * the binary contents of `block[i]` of the message
//! * the binary value of the counter `i` (or, the same value
//!   converted to ascii)
//!
//! Equally, the three values could be XOR-ed together. The "key"
//! parameters will be the same length as blocks, so this is
//! straightforward. However, when converting an integer from its
//! internal representation to something that can be XOR-ed with the
//! block, care needs to be taken to convert it into a portable
//! format. In particular, both the byte order and alignment must be
//! decided.
//!
//! # Encryption Functions: HMAC
//!
//! Hash-based Message Authentication Code (HMAC) is a technique that
//! uses a hash function and some other token known by both the sender
//! and receiver to authenticate a message as well as verify its
//! integrity.
//!
//! A HMAC construction can be used in place of a simple hash. There
//! are two potential benefits of doing so:
//!
//! * it allows the use of hash functions that are known to be weak,
//!   since the security of HMAC is only a function of the size of the
//!   shared HMAC token.
//!
//! * the HMAC token can be treated as a secret key, making the
//!   message undecodable without it. However, this would no longer be
//!   an AONT.
//!
//! It is still possible to use HMAC construction in either/both the
//! inner/outer encoding without breaking the "no-encryption" status
//! of AONT. Simply publish the HMAC token along with the public key,
//! or include it as part of a header/trailer for the transmitted
//! data.  It's also possible to use a random HMAC token, which can be
//! stored alongside or as part of `R`.
//!
//! # Encryption Functions: Block Ciphers
//!
//! It is also possible to use a symmetric encryption function (such
//! as AES) to implement `E()`. Note that only the output of the
//! encryption engine is used: the symmetric decryption function is
//! never called.
//!
//! Encryption routines can be used in several
//! [modes](https://en.wikipedia.org/wiki/Block_cipher_modes_of_operation)
//! including CBC (Cipher Block Chaining) or Counter mode.
//!
//! # Implementation
//!
//! I will implement this using a mix of high-level and low-level
//! interfaces. The high-level interfaces will implement the AONT
//! algorithm on messages and files. The next level down will allow
//! for easy parameterisation of the basic algorithm, such as allowing
//! a choice of encryption function.
//!
//! At the lowest level, I'll interact with the crypto and digest
//! libraries. For example, I might use those libraries to find out
//! what the block size should be (if it's not explicitly given to
//! us). I might also implement the two phases of the algorithm as
//! Digest algorithms (ie implement the Digest trait for them).


/// XOR block of data: *dst ^= *src, returning dst
pub fn xor_slice<'a> (dst : &'a mut [u8], src : &[u8]) -> &'a mut [u8] {

    // for now, require dst, src to be of equal length
    assert_eq!(dst.len(), src.len(),
	       "xor_slice: dst and src must be the same length" );

    // Can we use zip? Yes. Should also auto-vectorise.
    for (d,s) in dst.iter_mut().zip(src) {
        *d ^= s;
    }
    dst
}

use std::mem::size_of;
use rand::{thread_rng, Rng};
use sha1::{Sha1, Digest};

// First high-level prototype based on description above:
//
// * use SHA-1 for E() (locks in 160-bit = 20-byte block size)
// * concatenate arguments/parameters
// * use network (big-endian) order for bytes in i
// * operate on a "string" (actually &[u8] internally)

/// Encode a message using SHA-1
pub fn encode_sha1(message : &[u8], public : &[u8]) -> Box<[u8]> {

    // Actually, don't need to construct new hasher if we're only
    // calling associated method digest():
    //
    //    let hasher = Sha1::new();

    // get block size from hasher
    let blocksize = Sha1::output_size();
    assert_eq!(public.len(), blocksize,
	       "decode_sha1: public length {} != block size {}",
	       public.len(), blocksize );

    // allocate output buffer with extra block at the end for R ^ S
    let mut buffer = vec![0u8; message.len() + blocksize];

    // input buffer for hash(R, i)
    let mut r_in = vec![0u8; blocksize + size_of::<u32>()];
	  
    // generate R, storing it at the start of r_in
    let mut rng = thread_rng();
    for elem in r_in.iter_mut().take(blocksize) {
	*elem = rng.gen();
    }
    eprintln!("Generated random parameter: {:?}", r_in);

    // input buffer for hash(P, out[i] + i)
    let mut p_in = vec![0u8; blocksize * 2 + size_of::<u32>()];

    // place public key at start of p_in
    p_in[0..blocksize].copy_from_slice(public);

    // decide whether we need to pad input (for now, just panic)
    if message.len() % blocksize != 0 {
	panic!("Message is not a multiple of block size {}", blocksize);
    }

    // loop below calculates S, which will be used to mask R
    
    // use iterator to consume 16 bytes at a time
    //
    // TODO: change to use chunks_exact() in the loop and remainder()
    // afterwards (where padding can be implemented)
    let mut i : u32 = 1;
    let mut sum = vec![0u8; blocksize];

    for chunk in message.chunks(blocksize) {

	// copy message chunk into output buffer (will be masked later)
	//
	// It's probably better to just copy the full buffer outside the loop
	// 
	buffer[(i as usize  - 1) * blocksize..(i as usize * blocksize)].copy_from_slice(chunk);

	// both steps can be done in one pass
	// chunk  = in[i]
	// out[i] = chunk ^ hash(R, i)

	// concatenate i as big endian/network ordered bytes
	r_in[blocksize..].copy_from_slice(&i.to_be_bytes());


	// hasher returns a GenericArray, which converts to a slice
	// for xor_slice to work
	//
	// xor_slice also returns dst so we don't have to slice it again
	let dest =
	    xor_slice(&mut buffer[(i as usize  - 1) * blocksize..(i as usize * blocksize)], // destination
		      &Sha1::digest(&r_in));

	// concatenate out[i] (dest) to p_in
	p_in[blocksize..blocksize * 2].copy_from_slice(dest);

	// concatenate i as big endian
	p_in[blocksize * 2..].copy_from_slice(&i.to_be_bytes());
	
	// sum   ^= hash(P, out[i] + i)
	xor_slice(&mut sum, &Sha1::digest(&p_in));

	i += 1;
    }
    // append sum ^ R to output
    let last_block = (i as usize  - 1) * blocksize;
    xor_slice(&mut sum, &r_in[0..blocksize]);
    buffer[last_block..].copy_from_slice(&sum);

    // could also be explicit and say .into_boxed_slice():
    buffer.into()
}

/// Decode a message using SHA-1
pub fn decode_sha1(message : &[u8], public : &[u8]) -> Box<[u8]> {

    // Two passes required:
    // * apply E(P, received_block(i) + i) to recover R
    // * apply E(R,i) to recover message

    let blocksize = Sha1::output_size();
    let blocks = message.len() / blocksize;

    if message.len() % blocksize != 0 {
	panic!("Message is not a multiple of block size {}", blocksize);
    }
    assert_eq!(public.len(), blocksize,
	       "decode_sha1: public length {} != block size {}",
	       public.len(), blocksize );

    // output buffer one block shorter than input
    let mut buffer = vec![0u8; message.len() - blocksize];
    let mut r_in   = vec![0u8; blocksize + size_of::<u32>()];
    let mut p_in   = vec![0u8; blocksize * 2 + size_of::<u32>()];
    p_in[0..blocksize].copy_from_slice(public);

    let mut i : u32 = 1;
    let mut sum = vec![0u8; blocksize];

    // Pass 1: apply E(P, received_block(i) + i) to recover R
    for chunk in message.chunks(blocksize) {
	if i < blocks as u32 {	// chunk is part of message
	    p_in[blocksize..blocksize * 2].copy_from_slice(chunk);
	    p_in[blocksize * 2..].copy_from_slice(&i.to_be_bytes());
	    xor_slice(&mut sum, &Sha1::digest(&p_in));
	} else {		// last chunk = S xor R
	    r_in[0..blocksize].copy_from_slice(chunk);
	    xor_slice(&mut r_in[0..blocksize], &sum);
	    eprintln!("Recovered random parameter: {:?}", r_in);
	}
	i += 1;
    }

    // Pass 2: apply E(R,i) to recover message
    buffer[0..(blocks - 1) * blocksize].
	copy_from_slice(&message[0..(blocks - 1) * blocksize]);
    for i in 1..blocks {
	let index = (i as usize  - 1) * blocksize;
	let chunk = &mut buffer[index..index + blocksize];
	r_in[blocksize..].copy_from_slice(&(i as u32).to_be_bytes());
	xor_slice(chunk, &Sha1::digest(&r_in));
    }
    buffer.into()
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    #[should_panic]
    fn pass_in_str_19_as_bytes() {
	// should panic because 19 % 20 != 0
	let nineteen = "0123456789abcdef012";
	let slice = nineteen.as_bytes();
	let _boxed = encode_sha1(slice, slice);
    }

    #[test]
    fn pass_in_str_20_as_bytes() {
	let twenty = "0123456789abcdef0123";
	let slice  = twenty.as_bytes();
	let boxed  = encode_sha1(slice, slice);
	assert_ne!(*slice, *boxed);
    }

    #[test]
    fn same_20_bytes_back() {
	let twenty = "0123456789abcdef0123";
	let slice  = twenty.as_bytes();
	// also use twenty as public key
	let boxed  = encode_sha1(slice, slice);
	assert_ne!(*slice, *boxed);
	let back   = decode_sha1(&*boxed, slice);
	assert_eq!(slice, &*back);
    }

    #[test]
    fn same_40_bytes_back() {
	let forty = "0123456789abcdef01230123456789abcdef0123";
	let slice  = forty.as_bytes();
	// slice is now too long to be used as a key
	let boxed  = encode_sha1(slice, &slice[0..20]);
	assert_ne!(*slice, *boxed);
	let back   = decode_sha1(&*boxed, &slice[0..20]);
	assert_eq!(slice, &*back);
    }

    #[test]
    #[should_panic]
    fn public_encode_parameter() {
	let forty = "0123456789abcdef01230123456789abcdef0123";
	let slice  = forty.as_bytes();
	// slice is now too long to be used as a key
	let _oxed  = encode_sha1(slice, slice);
    }
    
    #[test]
    #[should_panic]
    fn public_decode_parameter() {
	let forty = "0123456789abcdef01230123456789abcdef0123";
	let slice  = forty.as_bytes();
	// slice is now too long to be used as a key
	let _boxed  = decode_sha1(slice, slice);
    }

}
