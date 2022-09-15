# Rent Vec
The idea behind this crate is to have a vector where each item has its owner, as if it was rented.

Every time you push something into a RentVec it will return you with a lease, and that is one way to access the underlying data.

```rs
use rent_vec::RentVec;
let mut vec = RentVec::new();

let mut lease = vec.push(1u32);

let mut item = lease.guard();
*item = 2;
```

The other way to access data is through the write guard and iterators, that guarantee that no lease is modifying its entry.

```rs
let mut guard = vec.guard();
guard.iter();
guard.iter_mut();
```

If an entry is removed, it will move an item from the back into its location, and mark the other as moved.

```rs
let mut lease = vec.push(10u32);
lease.remove();
```

Once a moved entry is accessed, the lease will become aware of the new location, and the moved entry can be freed.

## Why
If you need a StableVec that is as contiguous as possible. The leases are also guaranteed to be valid.

This was written in a morning to figure out what the concept could look like.

## Drawbacks
The access performance is worse, since it has to resolve moved entries. After the first resolution, it is O(1).

Push performance is also slower, since it has to search for freed entries amongst moved entries. If there aren't moved entries, it is O(1).