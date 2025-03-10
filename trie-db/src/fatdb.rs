// Copyright 2017, 2021 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{
	CError, DBValue, Query, Result, Trie, TrieDB, TrieDBIterator, TrieDBKeyIterator, TrieHash,
	TrieItem, TrieIterator, TrieKeyItem, TrieLayout,
};
use hash_db::{HashDBRef, Hasher};

use crate::{rstd::boxed::Box, MerkleValue, TrieDBBuilder};

/// A `Trie` implementation which hashes keys and uses a generic `HashDB` backing database.
/// Additionaly it stores inserted hash-key mappings for later retrieval.
///
/// Use it as a `Trie` or `TrieMut` trait object.
pub struct FatDB<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>
{
	raw: TrieDB<'db, 'cache, L, DB>,
}

impl<'db, 'cache, L, DB> FatDB<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	/// Create a new trie with the backing database `db` and empty `root`
	/// Initialise to the state entailed by the genesis block.
	/// This guarantees the trie is built correctly.
	pub fn new(db: &'db DB, root: &'db TrieHash<L>) -> Self {
		FatDB { raw: TrieDBBuilder::new(db, root).build() }
	}

	/// Get the backing database.
	pub fn db(&self) -> &dyn HashDBRef<L::Hash, DBValue> {
		self.raw.db()
	}
}

impl<'db, 'cache, L, DB> Trie<L> for FatDB<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	fn root(&self) -> &TrieHash<L> {
		self.raw.root()
	}

	fn contains(&self, key: &[u8]) -> Result<bool, TrieHash<L>, CError<L>> {
		self.raw.contains(L::Hash::hash(key).as_ref())
	}

	fn get_hash(&self, key: &[u8]) -> Result<Option<TrieHash<L>>, TrieHash<L>, CError<L>> {
		self.raw.get_hash(key)
	}

	fn get_with<Q: Query<L::Hash>>(
		&self,
		key: &[u8],
		query: Q,
	) -> Result<Option<Q::Item>, TrieHash<L>, CError<L>> {
		self.raw.get_with(L::Hash::hash(key).as_ref(), query)
	}

	fn lookup_first_descendant(
		&self,
		key: &[u8],
	) -> Result<Option<MerkleValue<TrieHash<L>>>, TrieHash<L>, CError<L>> {
		self.raw.lookup_first_descendant(key)
	}

	fn iter<'a>(
		&'a self,
	) -> Result<
		Box<dyn TrieIterator<L, Item = TrieItem<TrieHash<L>, CError<L>>> + 'a>,
		TrieHash<L>,
		CError<L>,
	> {
		FatDBIterator::<L, DB>::new(&self.raw).map(|iter| Box::new(iter) as Box<_>)
	}

	fn key_iter<'a>(
		&'a self,
	) -> Result<
		Box<dyn TrieIterator<L, Item = TrieKeyItem<TrieHash<L>, CError<L>>> + 'a>,
		TrieHash<L>,
		CError<L>,
	> {
		FatDBKeyIterator::<L, DB>::new(&self.raw).map(|iter| Box::new(iter) as Box<_>)
	}
}

/// Iterator over inserted pairs of key values.
pub struct FatDBIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	trie_iterator: TrieDBIterator<'db, 'cache, L, DB>,
	trie: &'db TrieDB<'db, 'cache, L, DB>,
}

impl<'db, 'cache, L, DB> FatDBIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	/// Creates new iterator.
	pub fn new(trie: &'db TrieDB<'db, 'cache, L, DB>) -> Result<Self, TrieHash<L>, CError<L>> {
		Ok(FatDBIterator { trie_iterator: TrieDBIterator::new(trie)?, trie })
	}
}

impl<'db, 'cache, L, DB> TrieIterator<L> for FatDBIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	fn seek(&mut self, key: &[u8]) -> Result<(), TrieHash<L>, CError<L>> {
		let hashed_key = L::Hash::hash(key);
		self.trie_iterator.seek(hashed_key.as_ref())
	}
}

impl<'db, 'cache, L, DB> Iterator for FatDBIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	type Item = TrieItem<TrieHash<L>, CError<L>>;

	fn next(&mut self) -> Option<Self::Item> {
		self.trie_iterator.next().map(|res| {
			res.map(|(hash, value)| {
				let aux_hash = L::Hash::hash(&hash);
				(
					self.trie.db().get(&aux_hash, Default::default()).expect("Missing fatdb hash"),
					value,
				)
			})
		})
	}
}

/// Iterator over inserted keys.
pub struct FatDBKeyIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	trie_iterator: TrieDBKeyIterator<'db, 'cache, L, DB>,
	trie: &'db TrieDB<'db, 'cache, L, DB>,
}

impl<'db, 'cache, L, DB> FatDBKeyIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	/// Creates new iterator.
	pub fn new(trie: &'db TrieDB<'db, 'cache, L, DB>) -> Result<Self, TrieHash<L>, CError<L>> {
		Ok(FatDBKeyIterator { trie_iterator: TrieDBKeyIterator::new(trie)?, trie })
	}
}

impl<'db, 'cache, L, DB> TrieIterator<L> for FatDBKeyIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	fn seek(&mut self, key: &[u8]) -> Result<(), TrieHash<L>, CError<L>> {
		let hashed_key = L::Hash::hash(key);
		self.trie_iterator.seek(hashed_key.as_ref())
	}
}

impl<'db, 'cache, L, DB> Iterator for FatDBKeyIterator<'db, 'cache, L, DB>
where
	L: TrieLayout,
	DB: HashDBRef<L::Hash, DBValue>,
{
	type Item = TrieKeyItem<TrieHash<L>, CError<L>>;

	fn next(&mut self) -> Option<Self::Item> {
		self.trie_iterator.next().map(|res| {
			res.map(|hash| {
				let aux_hash = L::Hash::hash(&hash);
				self.trie.db().get(&aux_hash, Default::default()).expect("Missing fatdb hash")
			})
		})
	}
}
