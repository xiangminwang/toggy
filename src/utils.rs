pub fn realign_unchecked<U, T>(data: &[U]) -> &[T] {
    unsafe { data.align_to().1 }
}