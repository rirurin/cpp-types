# cpp-types

[Banner goes here]

An (imperfect) reimplementation of parts of the C++ standard library in Rust, designed to directly read, modify and
create C++ data structures.

This crate was designed for use in *Metaphor Multiplayer*, a (in development) mod for Metaphor: ReFantazio which 
frequently requires reading and writing C++ stdlib structures created by the game. As such, this is best used in
game modding or other cases where you're working with a program without source code and have to work with data in-place.
(This also means that the vast majority of the work has been on the MSVC implementation)

## Supported Features

| Library Feature      | msvc             | libc++ (clang)   | libstdc++ (gcc)   |
|----------------------|------------------|------------------|-------------------|
| `std::string`        | ✅                | ⚠️ <sup>*3</sup>| ✅ <sup>*1</sup>  |
| `std::vector`        | ✅                | ❌                | ❌                 |
| `std::list`          | ✅                | ❌                | ❌                 |
| `std::tree`          | ✅                | ❌                | ❌                 |
| `std::unordered_map` | ✅                | ❌                | ❌                 |
| `std::optional`      | ✅                | ❌                | ❌                 |
| `std::mutex`         | ✅                | ❌                | ❌                 |
| `std::shared_ptr`    | ✅                | ❌                | ❌                 |
| `std::function`      | ⚠️ <sup>*2</sup> | ❌                | ❌                 |
| Hashing              | ✅ (FNV1A)        | ❌                | ❌                 |
| RTTI Type Info       | ✅                | ❌                | ❌                 |

*1: The first field (`self.ptr`) in the string is a non-zero pointer to the start of the string. In cases where it 
fits inline, this points to (`self.storage`). Since it's not possible (I think) for a method to return a self-referential 
struct on the stack, `new()` creates a dangling pointer for short strings and `set_pointers()` is required to set
the value properly. \
*2: Implementation is very incomplete as it only contains what's needed to run an existing callback. \
*3: Work in progress