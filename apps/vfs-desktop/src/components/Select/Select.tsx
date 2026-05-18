/**
 * Select - Reusable dropdown component
 * Uses global CSS variables from theme for consistent styling
 */
import React from 'react';
import './Select.css';

export interface SelectOption<T = string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SelectProps<T = string> {
  /** Label for the select (optional, for accessibility) */
  label?: string;

  /** Current selected value */
  value: T;

  /** Options to display */
  options: SelectOption<T>[];

  /** Change handler */
  onChange: (value: T) => void;

  /** Placeholder text */
  placeholder?: string;

  /** Whether the select is disabled */
  disabled?: boolean;

  /** Additional CSS classes */
  className?: string;

  /** Minimum width */
  minWidth?: string;

  /** Full width */
  fullWidth?: boolean;

  /** ID for the select element (for accessibility) */
  id?: string;

  /** Name attribute */
  name?: string;

  /** Click handler (for event propagation control) */
  onClick?: ((e: React.MouseEvent) => void) | undefined;
}

export function Select<T extends string | number = string>({
  label,
  value,
  options,
  onChange,
  placeholder,
  disabled = false,
  className = '',
  minWidth,
  fullWidth = false,
  id,
  name,
  onClick,
}: SelectProps<T>) {
  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const selectedValue = e.target.value;
    // Convert to proper type if needed
    const option = options.find((opt) => String(opt.value) === selectedValue);
    if (option) {
      onChange(option.value);
    }
  };

  const selectId =
    id ||
    (label ? `select-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);
  const labelId = label && selectId ? `${selectId}-label` : undefined;

  return (
    <div
      className={`vfs-select-wrapper ${fullWidth ? 'full-width' : ''} ${className}`}
      style={minWidth ? { minWidth } : undefined}
    >
      {label && (
        <label htmlFor={selectId} id={labelId} className="vfs-select-label">
          {label}
        </label>
      )}
      <div className="vfs-select-container" onClick={onClick}>
        <select
          id={selectId}
          name={name}
          value={String(value)}
          onChange={handleChange}
          disabled={disabled}
          className="vfs-select"
          aria-labelledby={labelId}
          aria-label={label || placeholder}
          onClick={onClick}
        >
          {placeholder && (
            <option value="" disabled>
              {placeholder}
            </option>
          )}
          {options.map((option) => (
            <option
              key={String(option.value)}
              value={String(option.value)}
              disabled={option.disabled}
            >
              {option.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
