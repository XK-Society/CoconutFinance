use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint as AnchorMint, TokenAccount, TokenInterface};
use anchor_spl::associated_token::AssociatedToken;
use spl_token_2022::state::Mint as SplMint;
use spl_token_2022::extension::{BaseStateWithExtensions, transfer_fee};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYsg476zPFsLnS");

#[program]
pub mod liquidity_pool {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, fee_basis_points: u16) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.authority = ctx.accounts.authority.key();
        pool.usdc_mint = ctx.accounts.usdc_mint.key();
        pool.lp_token_mint = ctx.accounts.lp_token_mint.key();
        pool.total_liquidity = 0;

        // Initialize transfer fee for LP tokens
        let mint_info = ctx.accounts.lp_token_mint.to_account_info();
        let mint = BaseStateWithExtensions::<SplMint>::unpack(&mint_info.data.borrow())?;
        let mut transfer_fee_config = mint.get_extension_mut::<transfer_fee::TransferFeeConfig>()?;
        transfer_fee_config.transfer_fee_basis_points = fee_basis_points;
        transfer_fee_config.maximum_fee = u64::MAX; // Set an appropriate maximum fee

        Ok(())
    }

    pub fn provide_liquidity(ctx: Context<ProvideLiquidity>, usdc_amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        // Transfer USDC from user to pool
        anchor_spl::token_2022::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.pool_usdc_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            usdc_amount,
        )?;

        // Calculate LP tokens to mint (1:1 ratio for simplicity)
        let lp_tokens_to_mint = usdc_amount;

        // Mint LP tokens to user
        anchor_spl::token_2022::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::MintTo {
                    mint: ctx.accounts.lp_token_mint.to_account_info(),
                    to: ctx.accounts.user_lp_token_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[b"pool", &[*ctx.bumps.get("pool").unwrap()]]],
            ),
            lp_tokens_to_mint,
        )?;

        pool.total_liquidity += usdc_amount;

        emit!(LiquidityProvidedEvent {
            user: ctx.accounts.user.key(),
            usdc_amount,
            lp_tokens_minted: lp_tokens_to_mint,
        });

        Ok(())
    }

    pub fn withdraw_liquidity(ctx: Context<WithdrawLiquidity>, lp_token_amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        // Calculate USDC to return (1:1 ratio for simplicity)
        let usdc_to_return = lp_token_amount;

        require!(pool.total_liquidity >= usdc_to_return, ErrorCode::InsufficientLiquidity);

        // Burn LP tokens from user
        anchor_spl::token_2022::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::Burn {
                    mint: ctx.accounts.lp_token_mint.to_account_info(),
                    from: ctx.accounts.user_lp_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            lp_token_amount,
        )?;

        // Transfer USDC from pool to user
        anchor_spl::token_2022::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_2022::Transfer {
                    from: ctx.accounts.pool_usdc_account.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[b"pool", &[*ctx.bumps.get("pool").unwrap()]]],
            ),
            usdc_to_return,
        )?;

        pool.total_liquidity -= usdc_to_return;

        emit!(LiquidityWithdrawnEvent {
            user: ctx.accounts.user.key(),
            lp_tokens_burned: lp_token_amount,
            usdc_returned: usdc_to_return,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + 32 + 32 + 32 + 8)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub usdc_mint: InterfaceAccount<'info, AnchorMint>,
    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = pool,
        token::token_program = token_program,
    )]
    pub lp_token_mint: InterfaceAccount<'info, AnchorMint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ProvideLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lp_token_mint: InterfaceAccount<'info, AnchorMint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = lp_token_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_lp_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lp_token_mint: InterfaceAccount<'info, AnchorMint>,
    #[account(mut)]
    pub user_lp_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct Pool {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub lp_token_mint: Pubkey,
    pub total_liquidity: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient liquidity in the pool")]
    InsufficientLiquidity,
}

#[event]
pub struct LiquidityProvidedEvent {
    pub user: Pubkey,
    pub usdc_amount: u64,
    pub lp_tokens_minted: u64,
}

#[event]
pub struct LiquidityWithdrawnEvent {
    pub user: Pubkey,
    pub lp_tokens_burned: u64,
    pub usdc_returned: u64,
}