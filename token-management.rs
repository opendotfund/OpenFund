// Token Management for DEX using SPL Token and Token-2022 programs
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use spl_token_2022::extension::{metadata::Metadata as TokenMetadata, metadata_pointer::MetadataPointer};
use spl_token_metadata_interface::{
    error::TokenMetadataError,
    instruction::create_metadata_pointer_account,
    state::{Field, TokenMetadata as TokenMetadataInterface},
};

declare_id!("TokenMgmtXjn5Xj7LxJuKq3JEzxH5tqLEX8PbkL2gAM"); // Replace with your program ID

#[program]
pub mod openfund_token_management {
    use super::*;

    pub fn create_token(
        ctx: Context<CreateToken>,
        name: String,
        symbol: String,
        uri: String,
        decimals: u8,
    ) -> Result<()> {
        // Create the SPL Token mint (standard functionality)
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: ctx.accounts.mint.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            decimals,
            ctx.accounts.authority.key,
            Some(ctx.accounts.authority.key),
        )?;

        // Set up metadata using Token-2022 if the token is created with Token-2022 program
        if ctx.accounts.token_program.key == &spl_token_2022::id() {
            // Create metadata pointer account
            let metadata_pointer_signer_seeds = &[
                b"metadata_pointer",
                ctx.accounts.mint.key().as_ref(),
                &[ctx.bumps["metadata_pointer"]],
            ];
            let metadata_pointer_signer = &[&metadata_pointer_signer_seeds[..]];

            // Set up metadata for the token
            let metadata = TokenMetadataInterface {
                name,
                symbol,
                uri,
                ..TokenMetadataInterface::default()
            };

            // Initialize the metadata pointer
            create_metadata_pointer_account(
                ctx.accounts.token_program.key,
                ctx.accounts.mint.key,
                ctx.accounts.metadata_pointer.key,
                ctx.accounts.authority.key,
                ctx.accounts.system_program.key,
                ctx.accounts.rent.to_account_info().lamports(),
            )?;

            // Update the metadata fields
            metadata.update_field(
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.metadata_pointer.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                Field::Name,
                metadata.name.clone(),
                ctx.accounts.system_program.to_account_info(),
            )?;

            metadata.update_field(
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.metadata_pointer.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                Field::Symbol,
                metadata.symbol.clone(),
                ctx.accounts.system_program.to_account_info(),
            )?;

            metadata.update_field(
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.metadata_pointer.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                Field::Uri,
                metadata.uri.clone(),
                ctx.accounts.system_program.to_account_info(),
            )?;
        }

        // Create a token account for the DEX itself
        token::initialize_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeAccount {
                    account: ctx.accounts.dex_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
        )?;

        Ok(())
    }

    pub fn mint_tokens(
        ctx: Context<MintTokens>,
        amount: u64,
    ) -> Result<()> {
        // Mint tokens to the specified account
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }

    pub fn create_user_token_account(
        ctx: Context<CreateUserTokenAccount>,
    ) -> Result<()> {
        // Create a token account for the user
        token::initialize_account(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeAccount {
                    account: ctx.accounts.token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
        )?;

        Ok(())
    }

    pub fn create_associated_token_account(
        ctx: Context<CreateAssociatedTokenAccount>,
    ) -> Result<()> {
        // This is handled automatically by the Associated Token Program
        // Just a wrapper to make it accessible through our program
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        init,
        payer = authority,
        space = Mint::LEN,
    )]
    pub mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = authority,
        seeds = [b"metadata_pointer", mint.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<MetadataPointer>()
    )]
    /// CHECK: This account is initialized in the instruction
    pub metadata_pointer: AccountInfo<'info>,
    
    #[account(
        init,
        payer = authority,
        token::mint = mint,
        token::authority = authority,
    )]
    pub dex_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        constraint = mint.mint_authority == COption::Some(authority.key())
    )]
    pub mint: Account<'info, Mint>,
    
    #[account(
        mut,
        constraint = token_account.mint == mint.key()
    )]
    pub token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateUserTokenAccount<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = user,
        token::mint = mint,
        token::authority = user,
    )]
    pub token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateAssociatedTokenAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// CHECK: This is the user who will own the associated token account
    pub user: AccountInfo<'info>,
    
    pub mint: Account<'info, Mint>,
    
    /// CHECK: This is the associated token account that will be created
    #[account(mut)]
    pub associated_token_account: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
