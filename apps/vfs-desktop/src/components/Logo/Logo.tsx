/**
 * Diaspor Logo - three-circle mark
 *
 * Matches the canonical brand SVG in diaspor/website/favicon.svg.
 * Three overlapping circles (indigo / violet / cyan) on a dark
 * rounded-rect background; monochrome variants for inline use.
 */

import "./Logo.css";

interface LogoProps {
    size?: number;
    className?: string;
}

const CIRCLES = [
    { cx: 22, cy: 22, fill: "#4338CA" },
    { cx: 40, cy: 22, fill: "#6366F1" },
    { cx: 24, cy: 40, fill: "#22D3EE" },
] as const;

/**
 * Main Logo - colored mark on dark rounded background.
 */
export function Logo({ size = 32, className = "" }: LogoProps) {
    return (
        <svg
            viewBox="0 0 64 64"
            xmlns="http://www.w3.org/2000/svg"
            width={size}
            height={size}
            className={`diaspor-logo ${className}`}
            role="img"
            aria-label="Diaspor"
        >
            <title>Diaspor</title>
            <rect width="64" height="64" rx="14" fill="#0a0b14" />
            {CIRCLES.map((c, i) => (
                <circle key={i} cx={c.cx} cy={c.cy} r="13" fill={c.fill} />
            ))}
        </svg>
    );
}

/**
 * Logo with wordmark.
 */
export function LogoWithText({ size = 32, className = "" }: LogoProps) {
    return (
        <div className={`logo-with-text ${className}`}>
            <Logo size={size} />
            <span className="logo-wordmark">Diaspor</span>
        </div>
    );
}

/**
 * Monochrome variant - inherits currentColor; no background.
 * Use inside menus, buttons, etc. where the mark must adapt to text color.
 */
export function LogoIcon({ size = 24, className = "" }: LogoProps) {
    return (
        <svg
            viewBox="0 0 64 64"
            xmlns="http://www.w3.org/2000/svg"
            width={size}
            height={size}
            className={`diaspor-logo-icon ${className}`}
            role="img"
            aria-label="Diaspor"
        >
            <title>Diaspor</title>
            {CIRCLES.map((c, i) => (
                <circle key={i} cx={c.cx} cy={c.cy} r="13" fill="currentColor" />
            ))}
        </svg>
    );
}

/**
 * Glyph - alias for monochrome variant at a different default size.
 */
export function LogoGlyph({ size = 24, className = "" }: LogoProps) {
    return <LogoIcon size={size} className={`diaspor-logo-glyph ${className}`} />;
}

/**
 * Animated variant - colors cycle through the brand palette.
 */
export function LogoAnimated({ size = 48, className = "" }: LogoProps) {
    return (
        <svg
            viewBox="0 0 64 64"
            xmlns="http://www.w3.org/2000/svg"
            width={size}
            height={size}
            className={`diaspor-logo diaspor-logo-animated ${className}`}
            role="img"
            aria-label="Diaspor"
        >
            <title>Diaspor</title>
            <rect width="64" height="64" rx="14" fill="#0a0b14" />
            <circle cx="22" cy="22" r="13" fill="#4338CA">
                <animate
                    attributeName="fill"
                    values="#4338CA;#6366F1;#22D3EE;#4338CA"
                    dur="3s"
                    repeatCount="indefinite"
                />
            </circle>
            <circle cx="40" cy="22" r="13" fill="#6366F1">
                <animate
                    attributeName="fill"
                    values="#6366F1;#22D3EE;#4338CA;#6366F1"
                    dur="3s"
                    repeatCount="indefinite"
                />
            </circle>
            <circle cx="24" cy="40" r="13" fill="#22D3EE">
                <animate
                    attributeName="fill"
                    values="#22D3EE;#4338CA;#6366F1;#22D3EE"
                    dur="3s"
                    repeatCount="indefinite"
                />
            </circle>
        </svg>
    );
}

export default Logo;
