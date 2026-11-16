/// Picks a const-generic size at runtime from a fixed set of literals.
///
/// A core sized by a const generic (`KeyedFair<N>`, `TenantFair<N>`) meets a
/// length that is only known once Ruby hands it over. `sized!` matches that
/// length against the sizes the binding compiles for and evaluates the body
/// with `N` bound as a `const N: usize` to the one that matched, so `N`
/// works anywhere a const generic does: `Core::<N>::new(..)`,
/// `std::array::from_fn::<_, N, _>(..)`, a non-capturing closure coerced to
/// a `fn` pointer. Any other length evaluates the `_` arm, typically a
/// `return Err(..)` naming the supported range.
///
/// ```
/// fn lanes(count: usize) -> Result<[u8; 2], String> {
///     Ok(matryona_machines::sized!(count, [1, 2, 3], |N| [N as u8, 0], _ => {
///         return Err(format!("lanes must be in 1..=3, not {count}"));
///     }))
/// }
///
/// assert_eq!(lanes(2), Ok([2, 0]));
/// assert_eq!(lanes(4), Err("lanes must be in 1..=3, not 4".to_owned()));
///
/// fn zeros(count: usize) -> Option<Box<[u32]>> {
///     Some(matryona_machines::sized!(count, [0, 1, 2], |N| {
///         Box::new(std::array::from_fn::<u32, N, _>(|_| 0)) as Box<[u32]>
///     }, _ => return None))
/// }
///
/// assert_eq!(zeros(2).as_deref(), Some(&[0, 0][..]));
/// assert!(zeros(3).is_none());
/// ```
///
/// The `|N|` names the constant; it is not a closure, the body runs in
/// place, so `return` and `?` in either arm leave the enclosing function.
/// The value is matched once and must be a `usize`.
#[macro_export]
macro_rules! sized {
    (
        $value:expr,
        [$($size:literal),+ $(,)?],
        |$n:ident| $body:expr,
        _ => $otherwise:expr $(,)?
    ) => {
        match $value {
            $($size => {
                const $n: usize = $size;
                $body
            })+
            _ => $otherwise,
        }
    };
}
