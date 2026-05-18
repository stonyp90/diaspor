/**
 * TokenBalance Component
 *
 * Displays user's token balance and allows purchasing plans
 */
import React, { useEffect, useState } from 'react';
import {
  TokenService,
  TokenBalanceUI,
  TokenPlanUI,
} from '../../services/token';
import './TokenBalance.css';

export function TokenBalance() {
  const [balance, setBalance] = useState<TokenBalanceUI | null>(null);
  const [plans, setPlans] = useState<TokenPlanUI[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [balanceData, plansData] = await Promise.all([
        TokenService.getBalance(),
        TokenService.getPlans(),
      ]);
      // Convert TokenBalance to TokenBalanceUI
      const balanceUI: TokenBalanceUI = {
        total: balanceData.total,
        used: balanceData.used,
        remaining: balanceData.remaining,
        resetDate: new Date(balanceData.reset_date),
        isPaid: balanceData.is_paid,
        planId: balanceData.plan_id,
      };
      setBalance(balanceUI);
      setPlans(plansData);
    } catch (error) {
      console.error('Failed to load token data:', error);
    } finally {
      setLoading(false);
    }
  };

  const handlePurchase = async (planId: string) => {
    const success = await TokenService.purchasePlan(planId);
    if (success) {
      await loadData();
    } else {
      alert('Payment integration not yet available. Please contact support.');
    }
  };

  if (loading) {
    return (
      <div className="token-balance">
        <div className="token-balance-loading">Loading token balance...</div>
      </div>
    );
  }

  if (!balance) {
    return null;
  }

  const usagePercentage =
    balance.total > 0 ? (balance.used / balance.total) * 100 : 0;
  const resetDate = new Date(balance.resetDate);
  const daysUntilReset = Math.ceil(
    (resetDate.getTime() - Date.now()) / (1000 * 60 * 60 * 24),
  );

  return (
    <div className="token-balance">
      <div className="token-balance-header">
        <h3>AI Tokens</h3>
        {balance.isPaid && <span className="token-badge paid">Pro</span>}
      </div>

      <div className="token-balance-stats">
        <div className="token-stat">
          <div className="token-stat-label">Remaining</div>
          <div className="token-stat-value">
            {balance.remaining.toLocaleString()}
          </div>
        </div>
        <div className="token-stat">
          <div className="token-stat-label">Used</div>
          <div className="token-stat-value">
            {balance.used.toLocaleString()}
          </div>
        </div>
        <div className="token-stat">
          <div className="token-stat-label">Total</div>
          <div className="token-stat-value">
            {balance.total === -1 ? '∞' : balance.total.toLocaleString()}
          </div>
        </div>
      </div>

      <div className="token-progress-bar-container">
        <div
          className="token-progress-bar"
          style={{
            width: `${Math.min(100, usagePercentage)}%`,
            backgroundColor:
              usagePercentage > 90
                ? 'var(--color-error)'
                : usagePercentage > 70
                  ? 'var(--color-warning)'
                  : 'var(--color-success)',
          }}
        />
      </div>

      <div className="token-reset-info">
        {daysUntilReset > 0 ? (
          <span>
            Resets in {daysUntilReset} day{daysUntilReset !== 1 ? 's' : ''}
          </span>
        ) : (
          <span>Resets today</span>
        )}
      </div>

      {!balance.isPaid && plans.length > 0 && (
        <div className="token-plans">
          <h4>Upgrade Plans</h4>
          <div className="token-plans-grid">
            {plans
              .filter((p) => p.id !== 'free')
              .map((plan) => (
                <div key={plan.id} className="token-plan-card">
                  <div className="token-plan-name">{plan.name}</div>
                  <div className="token-plan-price">
                    ${plan.price.toFixed(2)}
                    {plan.price > 0 && (
                      <span className="token-plan-period">/month</span>
                    )}
                  </div>
                  <div className="token-plan-tokens">
                    {plan.tokensPerMonth === -1
                      ? 'Unlimited'
                      : `${plan.tokensPerMonth.toLocaleString()} tokens/month`}
                  </div>
                  <ul className="token-plan-features">
                    {plan.features.map((feature, idx) => (
                      <li key={idx}>{feature}</li>
                    ))}
                  </ul>
                  <button
                    className="token-plan-button"
                    onClick={() => handlePurchase(plan.id)}
                  >
                    Upgrade
                  </button>
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  );
}
