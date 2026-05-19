/**
 * Select — Custom listbox dropdown with fully styled trigger AND panel.
 *
 * Replaces the native <select> with a <button> trigger + portal-style absolute
 * <div role="listbox"> so we own the option styling (the OS used to render the
 * popup as a light-gray native menu, which clashed with the dark theme).
 *
 * Public API (SelectProps, SelectOption, generic T) is unchanged — drop-in
 * replacement for the old native-select wrapper.
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

  /** ID for the trigger element (for accessibility) */
  id?: string;

  /** Name attribute (renders a hidden input so form serialization keeps working) */
  name?: string;

  /** Click handler (for event propagation control on the trigger) */
  onClick?: ((e: React.MouseEvent) => void) | undefined;
}

const TYPEAHEAD_RESET_MS = 500;

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
  const reactId = React.useId();
  const triggerId =
    id ||
    (label
      ? `select-${label.toLowerCase().replace(/\s+/g, '-')}`
      : `vfs-select-${reactId}`);
  const labelId = label ? `${triggerId}-label` : undefined;
  const panelId = `${triggerId}-panel`;
  const optionIdPrefix = `${triggerId}-opt-`;

  const [isOpen, setIsOpen] = React.useState(false);
  const [openAbove, setOpenAbove] = React.useState(false);
  const [activeIndex, setActiveIndex] = React.useState<number>(-1);

  const triggerRef = React.useRef<HTMLButtonElement | null>(null);
  const panelRef = React.useRef<HTMLDivElement | null>(null);
  const optionRefs = React.useRef<Array<HTMLButtonElement | null>>([]);
  const typeaheadBuf = React.useRef<string>('');
  const typeaheadTimer = React.useRef<number | null>(null);

  const selected = options.find((o) => o.value === value);
  const selectedIndex = options.findIndex((o) => o.value === value);

  // Compute flip-above when opening
  const computeFlip = React.useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return false;
    const rect = trigger.getBoundingClientRect();
    const PANEL_MAX = 320;
    const MARGIN = 8;
    const spaceBelow = window.innerHeight - rect.bottom - MARGIN;
    const spaceAbove = rect.top - MARGIN;
    // Flip above only if there's not enough room below AND more room above
    return spaceBelow < Math.min(PANEL_MAX, 160) && spaceAbove > spaceBelow;
  }, []);

  const open = React.useCallback(() => {
    if (disabled) return;
    setOpenAbove(computeFlip());
    setActiveIndex(selectedIndex >= 0 ? selectedIndex : firstEnabled(options));
    setIsOpen(true);
  }, [disabled, computeFlip, selectedIndex, options]);

  const close = React.useCallback(
    (focusTrigger = true) => {
      setIsOpen(false);
      typeaheadBuf.current = '';
      if (typeaheadTimer.current) {
        window.clearTimeout(typeaheadTimer.current);
        typeaheadTimer.current = null;
      }
      if (focusTrigger) triggerRef.current?.focus();
    },
    [],
  );

  const commit = React.useCallback(
    (idx: number) => {
      const opt = options[idx];
      if (!opt || opt.disabled) return;
      onChange(opt.value);
      close();
    },
    [options, onChange, close],
  );

  // Close on outside click
  React.useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      const t = e.target as Node;
      if (
        panelRef.current?.contains(t) ||
        triggerRef.current?.contains(t)
      ) {
        return;
      }
      setIsOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [isOpen]);

  // Close on Escape & window scroll/resize (reposition would be nice; for now close)
  React.useEffect(() => {
    if (!isOpen) return;
    const onScroll = () => setOpenAbove(computeFlip());
    window.addEventListener('resize', onScroll);
    return () => {
      window.removeEventListener('resize', onScroll);
    };
  }, [isOpen, computeFlip]);

  // Keep activeIndex's option focused when it changes
  React.useEffect(() => {
    if (!isOpen) return;
    const el = optionRefs.current[activeIndex];
    if (el) el.focus({ preventScroll: false });
  }, [isOpen, activeIndex]);

  const moveActive = React.useCallback(
    (dir: 1 | -1) => {
      const len = options.length;
      if (len === 0) return;
      let i = activeIndex;
      for (let n = 0; n < len; n++) {
        i = (i + dir + len) % len;
        if (!options[i]?.disabled) {
          setActiveIndex(i);
          return;
        }
      }
    },
    [activeIndex, options],
  );

  const moveToEnd = React.useCallback(
    (which: 'first' | 'last') => {
      const len = options.length;
      if (len === 0) return;
      const start = which === 'first' ? 0 : len - 1;
      const step = which === 'first' ? 1 : -1;
      for (let i = start; i >= 0 && i < len; i += step) {
        if (!options[i]?.disabled) {
          setActiveIndex(i);
          return;
        }
      }
    },
    [options],
  );

  const onTriggerKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (disabled) return;
    if (!isOpen) {
      if (
        e.key === 'ArrowDown' ||
        e.key === 'ArrowUp' ||
        e.key === 'Enter' ||
        e.key === ' '
      ) {
        e.preventDefault();
        open();
      }
      return;
    }
  };

  const onPanelKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      moveActive(1);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      moveActive(-1);
      return;
    }
    if (e.key === 'Home') {
      e.preventDefault();
      moveToEnd('first');
      return;
    }
    if (e.key === 'End') {
      e.preventDefault();
      moveToEnd('last');
      return;
    }
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      commit(activeIndex);
      return;
    }
    if (e.key === 'Tab') {
      // Treat Tab as commit-and-close so focus moves on naturally
      close(false);
      return;
    }
    // Type-ahead: printable single chars
    if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) {
      typeaheadBuf.current += e.key.toLowerCase();
      if (typeaheadTimer.current) {
        window.clearTimeout(typeaheadTimer.current);
      }
      typeaheadTimer.current = window.setTimeout(() => {
        typeaheadBuf.current = '';
      }, TYPEAHEAD_RESET_MS);

      const buf = typeaheadBuf.current;
      const len = options.length;
      // Start search from active+1 so repeated same-letter cycles through matches
      const startFrom = buf.length === 1 ? activeIndex + 1 : activeIndex;
      for (let n = 0; n < len; n++) {
        const idx = (startFrom + n) % len;
        const opt = options[idx];
        if (!opt || opt.disabled) continue;
        if (opt.label.toLowerCase().startsWith(buf)) {
          setActiveIndex(idx);
          return;
        }
      }
    }
  };

  const wrapperClass = [
    'vfs-select-wrapper',
    fullWidth ? 'full-width' : '',
    isOpen ? 'is-open' : '',
    openAbove ? 'open-above' : '',
    className,
  ]
    .filter(Boolean)
    .join(' ');

  const triggerLabel = selected?.label ?? placeholder ?? '';

  return (
    <div
      className={wrapperClass}
      style={minWidth ? { minWidth } : undefined}
    >
      {label && (
        <label htmlFor={triggerId} id={labelId} className="vfs-select-label">
          {label}
        </label>
      )}
      <div className="vfs-select-container" onClick={onClick}>
        <button
          ref={triggerRef}
          type="button"
          id={triggerId}
          className="vfs-select-trigger"
          role="combobox"
          aria-haspopup="listbox"
          aria-expanded={isOpen}
          aria-controls={panelId}
          aria-disabled={disabled || undefined}
          aria-labelledby={labelId}
          aria-label={label ? undefined : placeholder}
          aria-activedescendant={
            isOpen && activeIndex >= 0
              ? `${optionIdPrefix}${activeIndex}`
              : undefined
          }
          disabled={disabled}
          onClick={(e) => {
            onClick?.(e);
            if (e.defaultPrevented) return;
            if (isOpen) {
              close(false);
            } else {
              open();
            }
          }}
          onKeyDown={onTriggerKeyDown}
        >
          <span
            className={`vfs-select-value ${selected ? '' : 'placeholder'}`}
          >
            {triggerLabel}
          </span>
          <span className="vfs-select-chevron" aria-hidden="true" />
        </button>

        {name && (
          <input
            type="hidden"
            name={name}
            value={selected ? String(selected.value) : ''}
          />
        )}

        {isOpen && (
          <div
            ref={panelRef}
            id={panelId}
            role="listbox"
            aria-labelledby={labelId || triggerId}
            tabIndex={-1}
            className="vfs-select-panel"
            onKeyDown={onPanelKeyDown}
          >
            {options.map((opt, idx) => {
              const isSelected = opt.value === value;
              const isActive = idx === activeIndex;
              return (
                <button
                  key={`${String(opt.value)}-${idx}`}
                  ref={(el) => {
                    optionRefs.current[idx] = el;
                  }}
                  id={`${optionIdPrefix}${idx}`}
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  aria-disabled={opt.disabled || undefined}
                  tabIndex={-1}
                  className={[
                    'vfs-select-option',
                    isSelected ? 'is-selected' : '',
                    isActive ? 'is-active' : '',
                    opt.disabled ? 'is-disabled' : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (opt.disabled) return;
                    commit(idx);
                  }}
                  onMouseEnter={() => {
                    if (!opt.disabled) setActiveIndex(idx);
                  }}
                  disabled={opt.disabled}
                >
                  <span className="vfs-select-option-label">{opt.label}</span>
                  {isSelected && (
                    <span className="vfs-select-option-check" aria-hidden="true">
                      ✓
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function firstEnabled<T>(options: SelectOption<T>[]): number {
  for (let i = 0; i < options.length; i++) {
    if (!options[i]?.disabled) return i;
  }
  return -1;
}
