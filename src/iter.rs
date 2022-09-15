use std::sync::RwLockWriteGuard;

use crate::{Entry, InnerRentVec};

type Guard<'a, T> = RwLockWriteGuard<'a, InnerRentVec<T>>;

type FilterFn<T> = fn(&Entry<T>) -> Option<&T>;
type Filter<T, I> = std::iter::FilterMap<I, FilterFn<T>>;
type IterInner<'a, T> = Filter<T, std::slice::Iter<'a, Entry<T>>>;
pub struct Iter<'a, T> {
	inner: IterInner<'a, T>,
}
impl<'a, T> Iter<'a, T> {
	pub(super) fn new(guard: &'a Guard<'a, T>) -> Self {
		let slice = guard.items.iter();
		let inner = slice.filter_map(Entry::owned as FilterFn<T>);
		// No need to store the guard, since the reference will live as long as Self
		Iter { inner }
	}
}

impl<'a, T> Iterator for Iter<'a, T> {
	type Item = &'a T;
	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next()
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
	#[inline]
	fn fold<B, F>(self, init: B, f: F) -> B
	where
		Self: Sized,
		F: FnMut(B, Self::Item) -> B,
	{
		self.inner.fold(init, f)
	}
}

type FilterFnMut<T> = fn(&mut Entry<T>) -> Option<&mut T>;
type FilterMut<T, I> = std::iter::FilterMap<I, FilterFnMut<T>>;
type IterMutInner<'a, T> = FilterMut<T, std::slice::IterMut<'a, Entry<T>>>;
pub struct IterMut<'a, T> {
	inner: IterMutInner<'a, T>,
}
impl<'a, T> IterMut<'a, T> {
	pub(super) fn new(guard: &'a mut Guard<'_, T>) -> Self {
		let slice = guard.items.iter_mut();
		let inner = slice.filter_map(Entry::owned_mut as FilterFnMut<T>);
		// No need to store the guard, since the reference will live as long as Self
		IterMut { inner }
	}
}

impl<'a, T> Iterator for IterMut<'a, T> {
	type Item = &'a mut T;
	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next()
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
	#[inline]
	fn fold<B, F>(self, init: B, f: F) -> B
	where
		Self: Sized,
		F: FnMut(B, Self::Item) -> B,
	{
		self.inner.fold(init, f)
	}
}
