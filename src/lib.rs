//! # Rent Vec
//! The idea behind this crate is to have a vector where each item has its owner, as if it was rented.
//! 
//! Every time you push something into a RentVec it will return you with a lease, and that is one way to access the underlying data.
//! 
//! ```
//! use rent_vec::RentVec;
//! let mut vec = RentVec::new();
//! 
//! let mut lease = vec.push(1u32);
//! 
//! let mut item = lease.guard();
//! *item = 2;
//! ```
//! 
//! The other way to access data is through the write guard and iterators, that guarantee that no lease is modifying its entry.
//! 
//! ```
//! # use rent_vec::RentVec;
//! # let mut vec = RentVec::<u32>::new();
//! let mut guard = vec.guard();
//! guard.iter();
//! guard.iter_mut();
//! ```
//! 
//! If an entry is removed, it will move an item from the back into its location, and mark the other as moved.
//! 
//! ```
//! # use rent_vec::RentVec;
//! # let mut vec = RentVec::<u32>::new();
//! let mut lease = vec.push(10u32);
//! lease.remove();
//! ```
//! 
//! Once a moved entry is accessed, the lease will become aware of the new location, and the moved entry can be freed.
//! 
//! ## Why
//! If you need a StableVec that is as contiguous as possible. The leases are also guaranteed to be valid.
//! 
//! ## Drawbacks
//! The access performance is worse, since it has to resolve moved entries. After the first resolution, it is O(1).
//! 
//! Push performance is also slower, since it has to search for freed entries amongst moved entries. If there aren't moved entries, it is O(1).

pub mod iter;

use std::{
	fmt::Display,
	ops::{Deref, DerefMut},
	sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use iter::{Iter, IterMut};

fn write<T>(rwl: &RwLock<T>) -> RwLockWriteGuard<T> {
	match rwl.write() {
		Ok(g) => g,
		Err(g) => g.into_inner(),
	}
}

fn read<T>(rwl: &RwLock<T>) -> RwLockReadGuard<T> {
	match rwl.read() {
		Ok(g) => g,
		Err(g) => g.into_inner(),
	}
}

#[derive(Debug, Clone)]
pub enum Entry<T> {
	Empty,
	Owned(T),
	Moved(usize),
}
impl<T> Entry<T> {
	fn owned(&self) -> Option<&T> {
		if let Entry::Owned(t) = self {
			Some(t)
		} else {
			None
		}
	}
	fn owned_mut(&mut self) -> Option<&mut T> {
		if let Entry::Owned(t) = self {
			Some(t)
		} else {
			None
		}
	}
}
impl<T: Display> Display for Entry<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Entry::Empty => "Empty".fmt(f),
			Entry::Owned(t) => write!(f, "Owned({})", t),
			Entry::Moved(e) => write!(f, "Moved({})", e),
		}
	}
}

pub struct Lease<'v, T> {
	entry: usize,
	tenant: &'v RentVec<T>,
}
impl<'v, T> Lease<'v, T> {
	pub fn guard(&mut self) -> LeaseGuard<'_, T> {
		let guard = read(&self.tenant.lock);
		let item = guard.items.get(self.entry).and_then(|mut item| loop {
			match item {
				Entry::Empty => None?,
				Entry::Owned(t) => unsafe { break (t as *const T as *mut T).as_mut() },
				Entry::Moved(e) => {
					unsafe {
						let item = item as *const Entry<T> as *mut Entry<T>;
						*item = Entry::Empty
					};
					self.entry = *e;
					item = guard.items.get(self.entry)?;
				}
			}
		}).unwrap();
		// The guard can't be dropped here, or an iterator might write to this lease while it is being used
		LeaseGuard {
			item,
			_guard: guard,
		}
	}
	pub fn remove(self) {
		self.tenant.remove(self.entry);
	}
}
pub struct LeaseGuard<'l, T> {
	item: &'l mut T,
	_guard: RwLockReadGuard<'l, InnerRentVec<T>>,
}
impl<'l, T> Deref for LeaseGuard<'l, T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		self.item
	}
}
impl<'l, T> DerefMut for LeaseGuard<'l, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.item
	}
}

#[derive(Debug)]
struct InnerRentVec<T> {
	/// The index of the first [Entry::Moved] / [Entry::Empty].
	///
	/// As such, all items before the tail are [Entry::Owned].
	///
	/// And no items at or after the tail are [Entry::Owned].
	tail: usize,
	items: Vec<Entry<T>>,
}
impl<T> Default for InnerRentVec<T> {
	fn default() -> Self {
		Self {
			tail: 0,
			items: Vec::default(),
		}
	}
}

#[derive(Debug)]
pub struct RentVec<T> {
	lock: RwLock<InnerRentVec<T>>,
}

impl<T> Default for RentVec<T> {
	fn default() -> Self {
		Self {
			lock: RwLock::default(),
		}
	}
}

impl<T> RentVec<T> {
	pub fn new() -> Self {
		Self::default()
	}
	/// Removes an entry, and inserts another one from the back in its place.
	///
	/// The other entry's old location, in turn, is set to [Entry::Moved].
	fn remove(&self, entry: usize) -> Option<usize> {
		let mut guard = write(&self.lock);

		let mut replace = guard.tail - 1;

		let items = &mut guard.items;
		if entry == replace {
			items[entry] = Entry::Empty;
			guard.tail -= 1;
			return None;
		};

		// Searches for the last Owned entry before the tail
		loop {
			let item = &mut items[replace];
			match item {
				// Should never be reached, since all entries before the tail are Owned
				Entry::Empty | Entry::Moved(_) => {
					// Replace will never reach zero
					// The case where it would is handled above, when entry == replace
					// In that case, the entry removed is the last Owned
					replace -= 1
				}
				Entry::Owned(_) => {
					let item = std::mem::replace(item, Entry::Moved(entry));
					items[entry] = item;
					// Tail here is set to the first non-Owned entry
					// That we just set to Moved two lines above
					guard.tail = guard.tail.min(replace);
					break Some(replace);
				}
			}
		}
	}
	/// Inserts an entry at the first [Entry::Empty], or a new one if it doesn't exist.
	pub fn push(&self, item: T) -> Lease<'_, T> {
		let mut guard = write(&self.lock);
		let mut tail = guard.tail;
		let items = &mut guard.items;
		// If tail is equal to len, then there are no Empty entries
		let entry = (tail != items.len())
			.then(|| {
				// Searches for the first Empty entry after the tail
				loop {
					let item = &items[tail];
					match item {
						Entry::Empty => {
							break Some(tail);
						}
						// Should never be reached, since no entries after the tail are Owned
						Entry::Owned(_) => break None,
						Entry::Moved(_) => {
							// This case is possible if all entries past the tail are Moved
							if tail == items.len() {
								break None;
							};
							tail += 1
						}
					}
				}
			})
			.flatten();
		let entry = match entry {
			Some(entry) => {
				items[entry] = Entry::Owned(item);
				entry
			}
			None => {
				let entry = items.len();
				items.push(Entry::Owned(item));
				entry
			}
		};
		guard.tail = guard.tail.max(entry + 1);
		Lease {
			entry,
			tenant: self,
		}
	}
	pub fn guard(&self) -> RentVecGuard<'_, T> {
		let guard = write(&self.lock);
		RentVecGuard { guard }
	}
	pub fn shrink(&self) {
		let mut guard = write(&self.lock);
		while let Some(Entry::Empty) = guard.items.last() {
			guard.items.pop();
		}
		guard.items.shrink_to_fit();
		guard.tail = guard.tail.min(guard.items.len());
	}
}

pub struct RentVecGuard<'a, T> {
	guard: RwLockWriteGuard<'a, InnerRentVec<T>>,
}
impl<'a, T> RentVecGuard<'a, T> {
	pub fn iter(&self) -> Iter<'_, T> {
		Iter::new(&self.guard)
	}
	pub fn iter_mut(&mut self) -> IterMut<'_, T> {
		IterMut::new(&mut self.guard)
	}
}
