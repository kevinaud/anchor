extern crate std;
use anchor_lang::prelude::borsh::maybestd::io::Write;
use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
/// The static program ID
pub static ID: anchor_lang::solana_program::pubkey::Pubkey =
    anchor_lang::solana_program::pubkey::Pubkey::new_from_array([
        218u8, 7u8, 92u8, 178u8, 255u8, 94u8, 198u8, 129u8, 118u8, 19u8, 222u8, 83u8, 11u8, 105u8,
        42u8, 135u8, 53u8, 71u8, 119u8, 105u8, 218u8, 71u8, 67u8, 12u8, 189u8, 129u8, 84u8, 51u8,
        92u8, 74u8, 131u8, 39u8,
    ]);
/// Confirms that a given pubkey is equivalent to the program ID
pub fn check_id(id: &anchor_lang::solana_program::pubkey::Pubkey) -> bool {
    id == &ID
}
/// Returns the program ID
pub fn id() -> anchor_lang::solana_program::pubkey::Pubkey {
    ID
}
pub struct GenericsTest<'info, T, U, const N: usize>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
{
    pub non_generic: AccountInfo<'info>,
    pub generic: Account<'info, T>,
    pub const_generic: AccountLoader<'info, FooAccount<N>>,
    pub const_generic_loader: AccountLoader<'info, FooAccount<N>>,
    pub associated: Account<'info, Associated<U>>,
}
#[automatically_derived]
impl<'info, T, U, const N: usize> anchor_lang::Accounts<'info> for GenericsTest<'info, T, U, N>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        accounts: &mut &[anchor_lang::solana_program::account_info::AccountInfo<'info>],
        ix_data: &[u8],
    ) -> std::result::Result<Self, anchor_lang::solana_program::program_error::ProgramError> {
        let non_generic: AccountInfo =
            anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data)?;
        let generic: anchor_lang::Account<T> =
            anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data)?;
        let const_generic: anchor_lang::AccountLoader<FooAccount<N>> =
            anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data)?;
        let const_generic_loader: anchor_lang::AccountLoader<FooAccount<N>> =
            anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data)?;
        let associated: anchor_lang::Account<Associated<U>> =
            anchor_lang::Accounts::try_accounts(program_id, accounts, ix_data)?;
        Ok(GenericsTest {
            non_generic,
            generic,
            const_generic,
            const_generic_loader,
            associated,
        })
    }
}
#[automatically_derived]
impl<'info, T, U, const N: usize> anchor_lang::ToAccountInfos<'info>
    for GenericsTest<'info, T, U, N>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.non_generic.to_account_infos());
        account_infos.extend(self.generic.to_account_infos());
        account_infos.extend(self.const_generic.to_account_infos());
        account_infos.extend(self.const_generic_loader.to_account_infos());
        account_infos.extend(self.associated.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info, T, U, const N: usize> anchor_lang::ToAccountMetas for GenericsTest<'info, T, U, N>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
{
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.non_generic.to_account_metas(None));
        account_metas.extend(self.generic.to_account_metas(None));
        account_metas.extend(self.const_generic.to_account_metas(None));
        account_metas.extend(self.const_generic_loader.to_account_metas(None));
        account_metas.extend(self.associated.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info, T, U, const N: usize> anchor_lang::AccountsExit<'info> for GenericsTest<'info, T, U, N>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::solana_program::entrypoint::ProgramResult {
        Ok(())
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_generics_test {
    use super::*;
    use anchor_lang::prelude::borsh;
    pub struct GenericsTest {
        pub non_generic: anchor_lang::solana_program::pubkey::Pubkey,
        pub generic: anchor_lang::solana_program::pubkey::Pubkey,
        pub const_generic: anchor_lang::solana_program::pubkey::Pubkey,
        pub const_generic_loader: anchor_lang::solana_program::pubkey::Pubkey,
        pub associated: anchor_lang::solana_program::pubkey::Pubkey,
    }
    impl borsh::ser::BorshSerialize for GenericsTest
    where
        anchor_lang::solana_program::pubkey::Pubkey: borsh::ser::BorshSerialize,
        anchor_lang::solana_program::pubkey::Pubkey: borsh::ser::BorshSerialize,
        anchor_lang::solana_program::pubkey::Pubkey: borsh::ser::BorshSerialize,
        anchor_lang::solana_program::pubkey::Pubkey: borsh::ser::BorshSerialize,
        anchor_lang::solana_program::pubkey::Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.non_generic, writer)?;
            borsh::BorshSerialize::serialize(&self.generic, writer)?;
            borsh::BorshSerialize::serialize(&self.const_generic, writer)?;
            borsh::BorshSerialize::serialize(&self.const_generic_loader, writer)?;
            borsh::BorshSerialize::serialize(&self.associated, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for GenericsTest {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.non_generic,
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.generic,
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.const_generic,
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.const_generic_loader,
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    self.associated,
                    false,
                ),
            );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// `cpi::accounts` module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_generics_test {
    use super::*;
    pub struct GenericsTest<'info> {
        pub non_generic: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub generic: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub const_generic: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub const_generic_loader: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub associated: anchor_lang::solana_program::account_info::AccountInfo<'info>,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for GenericsTest<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    anchor_lang::Key::key(&self.non_generic),
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    anchor_lang::Key::key(&self.generic),
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    anchor_lang::Key::key(&self.const_generic),
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    anchor_lang::Key::key(&self.const_generic_loader),
                    false,
                ),
            );
            account_metas.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    anchor_lang::Key::key(&self.associated),
                    false,
                ),
            );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for GenericsTest<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos.push(anchor_lang::ToAccountInfo::to_account_info(
                &self.non_generic,
            ));
            account_infos.push(anchor_lang::ToAccountInfo::to_account_info(&self.generic));
            account_infos.push(anchor_lang::ToAccountInfo::to_account_info(
                &self.const_generic,
            ));
            account_infos.push(anchor_lang::ToAccountInfo::to_account_info(
                &self.const_generic_loader,
            ));
            account_infos.push(anchor_lang::ToAccountInfo::to_account_info(
                &self.associated,
            ));
            account_infos
        }
    }
}
#[repr(packed)]
pub struct FooAccount<const N: usize> {
    pub data: WrappedU8Array<N>,
}
#[automatically_derived]
impl<const N: usize> FooAccount<N> {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<const N: usize> ::core::marker::Copy for FooAccount<N> {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<const N: usize> ::core::clone::Clone for FooAccount<N> {
    #[inline]
    fn clone(&self) -> FooAccount<N> {
        {
            let _: ::core::clone::AssertParamIsClone<WrappedU8Array<N>>;
            *self
        }
    }
}
#[automatically_derived]
unsafe impl<const N: usize> anchor_lang::__private::bytemuck::Pod for FooAccount<N> {}
#[automatically_derived]
unsafe impl<const N: usize> anchor_lang::__private::bytemuck::Zeroable for FooAccount<N> {}
#[automatically_derived]
impl<const N: usize> anchor_lang::ZeroCopy for FooAccount<N> {}
#[automatically_derived]
impl<const N: usize> anchor_lang::Discriminator for FooAccount<N> {
    fn discriminator() -> [u8; 8] {
        [41, 191, 186, 219, 236, 67, 0, 167]
    }
}
#[automatically_derived]
impl<const N: usize> anchor_lang::AccountDeserialize for FooAccount<N> {
    fn try_deserialize(buf: &mut &[u8]) -> std::result::Result<Self, ProgramError> {
        if buf.len() < [41, 191, 186, 219, 236, 67, 0, 167].len() {
            return Err(anchor_lang::__private::ErrorCode::AccountDiscriminatorNotFound.into());
        }
        let given_disc = &buf[..8];
        if &[41, 191, 186, 219, 236, 67, 0, 167] != given_disc {
            return Err(anchor_lang::__private::ErrorCode::AccountDiscriminatorMismatch.into());
        }
        Self::try_deserialize_unchecked(buf)
    }
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> std::result::Result<Self, ProgramError> {
        let data: &[u8] = &buf[8..];
        let account = anchor_lang::__private::bytemuck::from_bytes(data);
        Ok(*account)
    }
}
#[automatically_derived]
impl<const N: usize> anchor_lang::Owner for FooAccount<N> {
    fn owner() -> Pubkey {
        crate::ID
    }
}
pub struct Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    pub data: T,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::default::Default> ::core::default::Default for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    #[inline]
    fn default() -> Associated<T> {
        Associated {
            data: ::core::default::Default::default(),
        }
    }
}
impl<T> borsh::ser::BorshSerialize for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
    T: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.data, writer)?;
        Ok(())
    }
}
impl<T> borsh::de::BorshDeserialize for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
    T: borsh::BorshDeserialize,
{
    fn deserialize(buf: &mut &[u8]) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            data: borsh::BorshDeserialize::deserialize(buf)?,
        })
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::clone::Clone> ::core::clone::Clone for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    #[inline]
    fn clone(&self) -> Associated<T> {
        match *self {
            Associated {
                data: ref __self_0_0,
            } => Associated {
                data: ::core::clone::Clone::clone(&(*__self_0_0)),
            },
        }
    }
}
#[automatically_derived]
impl<T> anchor_lang::AccountSerialize for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    fn try_serialize<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> std::result::Result<(), ProgramError> {
        writer
            .write_all(&[141, 87, 143, 75, 26, 10, 156, 28])
            .map_err(|_| anchor_lang::__private::ErrorCode::AccountDidNotSerialize)?;
        AnchorSerialize::serialize(self, writer)
            .map_err(|_| anchor_lang::__private::ErrorCode::AccountDidNotSerialize)?;
        Ok(())
    }
}
#[automatically_derived]
impl<T> anchor_lang::AccountDeserialize for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    fn try_deserialize(buf: &mut &[u8]) -> std::result::Result<Self, ProgramError> {
        if buf.len() < [141, 87, 143, 75, 26, 10, 156, 28].len() {
            return Err(anchor_lang::__private::ErrorCode::AccountDiscriminatorNotFound.into());
        }
        let given_disc = &buf[..8];
        if &[141, 87, 143, 75, 26, 10, 156, 28] != given_disc {
            return Err(anchor_lang::__private::ErrorCode::AccountDiscriminatorMismatch.into());
        }
        Self::try_deserialize_unchecked(buf)
    }
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> std::result::Result<Self, ProgramError> {
        let mut data: &[u8] = &buf[8..];
        AnchorDeserialize::deserialize(&mut data)
            .map_err(|_| anchor_lang::__private::ErrorCode::AccountDidNotDeserialize.into())
    }
}
#[automatically_derived]
impl<T> anchor_lang::Discriminator for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    fn discriminator() -> [u8; 8] {
        [141, 87, 143, 75, 26, 10, 156, 28]
    }
}
#[automatically_derived]
impl<T> anchor_lang::Owner for Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Default,
{
    fn owner() -> Pubkey {
        crate::ID
    }
}
pub struct WrappedU8Array<const N: usize>(u8);
#[automatically_derived]
#[allow(unused_qualifications)]
impl<const N: usize> ::core::marker::Copy for WrappedU8Array<N> {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<const N: usize> ::core::clone::Clone for WrappedU8Array<N> {
    #[inline]
    fn clone(&self) -> WrappedU8Array<N> {
        {
            let _: ::core::clone::AssertParamIsClone<u8>;
            *self
        }
    }
}
impl<const N: usize> BorshSerialize for WrappedU8Array<N> {
    fn serialize<W: Write>(&self, _writer: &mut W) -> borsh::maybestd::io::Result<()> {
        ::core::panicking::panic("not yet implemented")
    }
}
impl<const N: usize> BorshDeserialize for WrappedU8Array<N> {
    fn deserialize(_buf: &mut &[u8]) -> borsh::maybestd::io::Result<Self> {
        ::core::panicking::panic("not yet implemented")
    }
}
impl<const N: usize> Owner for WrappedU8Array<N> {
    fn owner() -> Pubkey {
        crate::ID
    }
}
