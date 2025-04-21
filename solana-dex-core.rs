// Core AMM DEX contract for Solana using Anchor framework
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use std::ops::Div;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"); // Replace with your program ID

#[program]
pub mod openfund_dex {
    use super::*;

    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        fee_numerator: u64,
        fee_denominator: u64,
    ) -> Result<()> {
        require!(fee_denominator > 0, ErrorCode::InvalidFee);
        require!(fee_numerator < fee_denominator, ErrorCode::InvalidFee);

        let pool = &mut ctx.accounts.pool;
        pool.token_a_mint = ctx.accounts.token_a_mint.key();
        pool.token_b_mint = ctx.accounts.token_b_mint.key();
        pool.token_a_account = ctx.accounts.token_a_account.key();
        pool.token_b_account = ctx.accounts.token_b_account.key();
        pool.lp_mint = ctx.accounts.lp_mint.key();
        pool.authority = ctx.accounts.authority.key();
        pool.fee_numerator = fee_numerator;
        pool.fee_denominator = fee_denominator;
        pool.bump = *ctx.bumps.get("pool").unwrap();

        Ok(())
    }

    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        amount_a: u64,
        amount_b: u64,
        min_lp_tokens: u64,
    ) -> Result<()> {
        // Ensure provided amounts are valid
        require!(amount_a > 0 && amount_b > 0, ErrorCode::InvalidAmount);

        let pool = &ctx.accounts.pool;
        let token_a_supply = ctx.accounts.token_a_account.amount;
        let token_b_supply = ctx.accounts.token_b_account.amount;
        let lp_supply = ctx.accounts.lp_mint.supply;

        // Calculate LP tokens to mint
        let lp_tokens = if lp_supply == 0 {
            // Initial liquidity - Use square root of product
            (amount_a as f64 * amount_b as f64).sqrt() as u64
        } else {
            // Calculate based on the ratio of existing reserves
            let lp_amount_a = (amount_a as u128)
                .checked_mul(lp_supply as u128)
                .unwrap()
                .div(token_a_supply as u128) as u64;
            
            let lp_amount_b = (amount_b as u128)
                .checked_mul(lp_supply as u128)
                .unwrap()
                .div(token_b_supply as u128) as u64;
            
            // Use the minimum to prevent manipulation
            std::cmp::min(lp_amount_a, lp_amount_b)
        };

        // Ensure the minimum LP tokens requirement is met
        require!(lp_tokens >= min_lp_tokens, ErrorCode::SlippageExceeded);

        // Transfer tokens from user to pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_a.to_account_info(),
                    to: ctx.accounts.token_a_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_a,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_b.to_account_info(),
                    to: ctx.accounts.token_b_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_b,
        )?;

        // Mint LP tokens to user
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    to: ctx.accounts.user_lp_token.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority", 
                    pool.bump.to_le_bytes().as_ref()
                ][..]],
            ),
            lp_tokens,
        )?;

        Ok(())
    }

    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_amount: u64,
        min_amount_a: u64,
        min_amount_b: u64,
    ) -> Result<()> {
        require!(lp_amount > 0, ErrorCode::InvalidAmount);
        
        let pool = &ctx.accounts.pool;
        let token_a_supply = ctx.accounts.token_a_account.amount;
        let token_b_supply = ctx.accounts.token_b_account.amount;
        let lp_supply = ctx.accounts.lp_mint.supply;
        
        // Calculate token amounts to return
        let amount_a = (lp_amount as u128)
            .checked_mul(token_a_supply as u128)
            .unwrap()
            .div(lp_supply as u128) as u64;
            
        let amount_b = (lp_amount as u128)
            .checked_mul(token_b_supply as u128)
            .unwrap()
            .div(lp_supply as u128) as u64;
            
        // Check slippage
        require!(amount_a >= min_amount_a, ErrorCode::SlippageExceeded);
        require!(amount_b >= min_amount_b, ErrorCode::SlippageExceeded);
        
        // Burn LP tokens
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    from: ctx.accounts.user_lp_token.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            lp_amount,
        )?;
        
        // Transfer tokens from pool to user
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_a_account.to_account_info(),
                    to: ctx.accounts.user_token_a.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority", 
                    pool.bump.to_le_bytes().as_ref()
                ][..]],
            ),
            amount_a,
        )?;
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_b_account.to_account_info(),
                    to: ctx.accounts.user_token_b.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority", 
                    pool.bump.to_le_bytes().as_ref()
                ][..]],
            ),
            amount_b,
        )?;
        
        Ok(())
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        require!(amount_in > 0, ErrorCode::InvalidAmount);
        
        let pool = &ctx.accounts.pool;
        
        // Calculate the fee
        let fee = (amount_in as u128)
            .checked_mul(pool.fee_numerator as u128)
            .unwrap()
            .div(pool.fee_denominator as u128) as u64;
            
        // Calculate the amount in after fee
        let amount_in_after_fee = amount_in.checked_sub(fee).unwrap();
        
        // Determine which token is being swapped in/out
        let (reserve_in, reserve_out) = if ctx.accounts.user_token_in.mint == pool.token_a_mint {
            (ctx.accounts.token_a_account.amount, ctx.accounts.token_b_account.amount)
        } else {
            (ctx.accounts.token_b_account.amount, ctx.accounts.token_a_account.amount)
        };
        
        // Calculate amount out using constant product formula: (x * y = k)
        // new_reserve_out = (reserve_in * reserve_out) / (reserve_in + amount_in_after_fee)
        // amount_out = reserve_out - new_reserve_out
        
        let new_reserve_in = reserve_in.checked_add(amount_in_after_fee).unwrap();
        let product = (reserve_in as u128).checked_mul(reserve_out as u128).unwrap();
        let new_reserve_out = product.div(new_reserve_in as u128) as u64;
        let amount_out = reserve_out.checked_sub(new_reserve_out).unwrap();
        
        // Check slippage
        require!(amount_out >= min_amount_out, ErrorCode::SlippageExceeded);
        
        // Transfer token in from user to pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_in.to_account_info(),
                    to: if ctx.accounts.user_token_in.mint == pool.token_a_mint {
                        ctx.accounts.token_a_account.to_account_info()
                    } else {
                        ctx.accounts.token_b_account.to_account_info()
                    },
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;
        
        // Transfer token out from pool to user
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: if ctx.accounts.user_token_out.mint == pool.token_a_mint {
                        ctx.accounts.token_a_account.to_account_info()
                    } else {
                        ctx.accounts.token_b_account.to_account_info()
                    },
                    to: ctx.accounts.user_token_out.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority", 
                    pool.bump.to_le_bytes().as_ref()
                ][..]],
            ),
            amount_out,
        )?;
        
        Ok(())
    }
}

// Account structures for the AMM pool
#[account]
pub struct Pool {
    pub token_a_mint: Pubkey,     // Mint address of token A
    pub token_b_mint: Pubkey,     // Mint address of token B
    pub token_a_account: Pubkey,  // Pool's token A account
    pub token_b_account: Pubkey,  // Pool's token B account
    pub lp_mint: Pubkey,          // LP token mint address
    pub authority: Pubkey,        // Authority that can modify the pool
    pub fee_numerator: u64,       // Numerator for fee calculation (e.g., 3 for 0.3%)
    pub fee_denominator: u64,     // Denominator for fee calculation (e.g., 1000 for 0.3%)
    pub bump: u8,                 // PDA bump seed
}

// Context for initializing a new pool
#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init, 
        payer = authority, 
        space = 8 + std::mem::size_of::<Pool>(),
        seeds = [
            b"pool".as_ref(),
            token_a_mint.key().as_ref(),
            token_b_mint.key().as_ref(),
        ],
        bump
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        seeds = [b"pool_authority".as_ref(), pool.key().as_ref()],
        bump,
    )]
    /// CHECK: This is a PDA used as the authority for the pool's token accounts
    pub pool_authority: AccountInfo<'info>,
    
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = authority,
        token::mint = token_a_mint,
        token::authority = pool_authority,
    )]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = authority,
        token::mint = token_b_mint,
        token::authority = pool_authority,
    )]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = pool_authority,
    )]
    pub lp_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

// Context for adding liquidity to a pool
#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            b"pool".as_ref(),
            pool.token_a_mint.as_ref(),
            pool.token_b_mint.as_ref(),
        ],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        seeds = [b"pool_authority".as_ref(), pool.key().as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that acts as the pool authority
    pub pool_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = token_a_account.key() == pool.token_a_account
    )]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token_b_account.key() == pool.token_b_account
    )]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = lp_mint.key() == pool.lp_mint
    )]
    pub lp_mint: Account<'info, Mint>,
    
    #[account(
        mut,
        constraint = user_token_a.mint == pool.token_a_mint,
        constraint = user_token_a.owner == user.key()
    )]
    pub user_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_b.mint == pool.token_b_mint,
        constraint = user_token_b.owner == user.key()
    )]
    pub user_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_lp_token.mint == pool.lp_mint,
        constraint = user_lp_token.owner == user.key()
    )]
    pub user_lp_token: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

// Context for removing liquidity from a pool
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            b"pool".as_ref(),
            pool.token_a_mint.as_ref(),
            pool.token_b_mint.as_ref(),
        ],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        seeds = [b"pool_authority".as_ref(), pool.key().as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that acts as the pool authority
    pub pool_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = token_a_account.key() == pool.token_a_account
    )]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token_b_account.key() == pool.token_b_account
    )]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = lp_mint.key() == pool.lp_mint
    )]
    pub lp_mint: Account<'info, Mint>,
    
    #[account(
        mut,
        constraint = user_token_a.mint == pool.token_a_mint,
        constraint = user_token_a.owner == user.key()
    )]
    pub user_token_a: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_token_b.mint == pool.token_b_mint,
        constraint = user_token_b.owner == user.key()
    )]
    pub user_token_b: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = user_lp_token.mint == pool.lp_mint,
        constraint = user_lp_token.owner == user.key()
    )]
    pub user_lp_token: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

// Context for swapping tokens
#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        seeds = [
            b"pool".as_ref(),
            pool.token_a_mint.as_ref(),
            pool.token_b_mint.as_ref(),
        ],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        seeds = [b"pool_authority".as_ref(), pool.key().as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that acts as the pool authority
    pub pool_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = token_a_account.key() == pool.token_a_account
    )]
    pub token_a_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = token_b_account.key() == pool.token_b_account
    )]
    pub token_b_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = (user_token_in.mint == pool.token_a_mint || user_token_in.mint == pool.token_b_mint),
        constraint = user_token_in.owner == user.key()
    )]
    pub user_token_in: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = (user_token_out.mint == pool.token_a_mint || user_token_out.mint == pool.token_b_mint),
        constraint = user_token_in.mint != user_token_out.mint,
        constraint = user_token_out.owner == user.key()
    )]
    pub user_token_out: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid fee parameters")]
    InvalidFee,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
}
