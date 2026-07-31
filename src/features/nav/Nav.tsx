import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
  type Ref,
} from "react";
import "./Nav.css";

const NARROW_QUERY = "(max-width: 560px)";

export interface MainNavItem<Id extends string = string> {
  id: Id;
  label: string;
  icon: ReactNode;
  badge?: ReactNode;
  badgeLabel?: string;
  disabled?: boolean;
}

export interface MainNavBrand {
  mark: ReactNode;
  name: ReactNode;
  subtitle?: ReactNode;
  ariaLabel?: string;
}

export interface MainNavImportAction {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}

export interface MainNavPrivacy {
  title: ReactNode;
  description?: ReactNode;
}

export interface MainNavProps<Id extends string = string> {
  activeId: Id;
  items: readonly MainNavItem<Id>[];
  settingsItem: MainNavItem<Id>;
  onNavigate: (id: Id) => void;
  brand: MainNavBrand;
  importAction?: MainNavImportAction;
  privacy?: MainNavPrivacy;
  ariaLabel: string;
  menuLabel: string;
  closeMenuLabel: string;
  className?: string;
}

type IndicatorGeometry = {
  x: number;
  y: number;
  width: number;
  height: number;
  scaleX: number;
  scaleY: number;
  origin: string;
  visible: boolean;
  phase: "settled" | "traveling";
};

const INITIAL_INDICATOR: IndicatorGeometry = {
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  scaleX: 1,
  scaleY: 1,
  origin: "center",
  visible: false,
  phase: "settled",
};

function viewportIsNarrow() {
  if (typeof window === "undefined") return false;
  if (typeof window.matchMedia === "function") {
    return window.matchMedia(NARROW_QUERY).matches;
  }
  return window.innerWidth <= 560;
}

function useNarrowViewport() {
  const [isNarrow, setIsNarrow] = useState(viewportIsNarrow);

  useEffect(() => {
    const update = () => setIsNarrow(viewportIsNarrow());
    const media = typeof window.matchMedia === "function"
      ? window.matchMedia(NARROW_QUERY)
      : null;

    if (media && typeof media.addEventListener === "function") {
      media.addEventListener("change", update);
    } else {
      media?.addListener(update);
    }
    window.addEventListener("resize", update);

    return () => {
      if (media && typeof media.removeEventListener === "function") {
        media.removeEventListener("change", update);
      } else {
        media?.removeListener(update);
      }
      window.removeEventListener("resize", update);
    };
  }, []);

  return isNarrow;
}

function accessibleItemLabel<Id extends string>(item: MainNavItem<Id>) {
  const badge = item.badgeLabel
    ?? (typeof item.badge === "string" || typeof item.badge === "number"
      ? String(item.badge)
      : "");
  return badge ? `${item.label} ${badge}` : item.label;
}

interface NavDestinationProps<Id extends string> {
  item: MainNavItem<Id>;
  active: boolean;
  tooltipId: string;
  tabIndex?: number;
  buttonRef: Ref<HTMLButtonElement>;
  onChoose: (id: Id) => void;
}

function NavDestination<Id extends string>({
  item,
  active,
  tooltipId,
  tabIndex,
  buttonRef,
  onChoose,
}: NavDestinationProps<Id>) {
  return <span className="cylune-nav__destination">
    <button
      ref={buttonRef}
      type="button"
      className="cylune-nav__item"
      aria-current={active ? "page" : undefined}
      aria-describedby={tooltipId}
      aria-label={accessibleItemLabel(item)}
      disabled={item.disabled}
      tabIndex={tabIndex}
      onClick={() => onChoose(item.id)}
    >
      <span className="cylune-nav__icon" aria-hidden="true">{item.icon}</span>
      <span className="cylune-nav__label" aria-hidden="true">{item.label}</span>
      {item.badge !== undefined ? <span className="cylune-nav__badge" aria-hidden="true">{item.badge}</span> : null}
    </button>
    <span id={tooltipId} className="cylune-nav__tooltip" role="tooltip">{item.label}</span>
  </span>;
}

function MenuIcon({ open }: { open: boolean }) {
  return <svg aria-hidden="true" viewBox="0 0 24 24">
    {open
      ? <path d="M6.7 6.7 17.3 17.3M17.3 6.7 6.7 17.3" />
      : <path d="M4.5 7.5h15M4.5 12h15M4.5 16.5h15" />}
  </svg>;
}

function focusableElements(root: HTMLElement) {
  return Array.from(root.querySelectorAll<HTMLElement>(
    "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]",
  )).filter((element) => element.tabIndex >= 0 && element.getAttribute("aria-hidden") !== "true");
}

export function MainNav<Id extends string>({
  activeId,
  items,
  settingsItem,
  onNavigate,
  brand,
  importAction,
  privacy,
  ariaLabel,
  menuLabel,
  closeMenuLabel,
  className = "",
}: MainNavProps<Id>) {
  const idPrefix = useId();
  const drawerId = `${idPrefix}-drawer`;
  const navigation = useRef<HTMLElement>(null);
  const drawer = useRef<HTMLDivElement>(null);
  const menuButton = useRef<HTMLButtonElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const itemElements = useRef(new Map<Id, HTMLButtonElement>());
  const previousGeometry = useRef<Pick<IndicatorGeometry, "x" | "y" | "width" | "height"> | null>(null);
  const settleTimer = useRef<number | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [indicator, setIndicator] = useState<IndicatorGeometry>(INITIAL_INDICATOR);
  const isNarrow = useNarrowViewport();
  const closedTabIndex = isNarrow && !drawerOpen ? -1 : undefined;

  const rememberItem = useCallback((id: Id, element: HTMLButtonElement | null) => {
    if (element) itemElements.current.set(id, element);
    else itemElements.current.delete(id);
  }, []);

  const closeDrawer = useCallback(() => setDrawerOpen(false), []);

  const choose = useCallback((id: Id) => {
    onNavigate(id);
    if (isNarrow) closeDrawer();
  }, [closeDrawer, isNarrow, onNavigate]);

  const runImport = useCallback(() => {
    importAction?.onClick();
    if (isNarrow) closeDrawer();
  }, [closeDrawer, importAction, isNarrow]);

  const toggleDrawer = () => {
    if (!drawerOpen) opener.current = menuButton.current;
    setDrawerOpen((current) => !current);
  };

  useEffect(() => {
    if (!isNarrow && drawerOpen) setDrawerOpen(false);
  }, [drawerOpen, isNarrow]);

  useEffect(() => {
    if (!isNarrow || !drawerOpen || !drawer.current) return;
    const drawerElement = drawer.current;
    const selected = itemElements.current.get(activeId);
    (selected ?? focusableElements(drawerElement)[0])?.focus({ preventScroll: true });

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeDrawer();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = focusableElements(drawerElement);
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !drawerElement.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !drawerElement.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      if (opener.current?.isConnected) opener.current.focus();
    };
  }, [activeId, closeDrawer, drawerOpen, isNarrow]);

  useLayoutEffect(() => {
    const navElement = navigation.current;
    const activeElement = itemElements.current.get(activeId);
    if (!navElement || !activeElement) return;

    const updateIndicator = () => {
      const navRect = navElement.getBoundingClientRect();
      const itemRect = activeElement.getBoundingClientRect();
      const next = {
        x: itemRect.left - navRect.left || activeElement.offsetLeft,
        y: itemRect.top - navRect.top || activeElement.offsetTop,
        width: itemRect.width || activeElement.offsetWidth || 40,
        height: itemRect.height || activeElement.offsetHeight || 44,
      };
      const previous = previousGeometry.current;
      const deltaX = previous ? next.x - previous.x : 0;
      const deltaY = previous ? next.y - previous.y : 0;
      const distance = Math.hypot(deltaX, deltaY);
      const traveling = Boolean(previous && distance > 2);
      const verticalTravel = Math.abs(deltaY) >= Math.abs(deltaX);
      const stretch = traveling ? Math.min(1.2, 1.08 + distance / 900) : 1;

      if (settleTimer.current !== null) window.clearTimeout(settleTimer.current);
      setIndicator({
        ...next,
        scaleX: traveling && !verticalTravel ? stretch : traveling ? 0.97 : 1,
        scaleY: traveling && verticalTravel ? stretch : traveling ? 0.97 : 1,
        origin: !traveling
          ? "center"
          : verticalTravel
            ? (deltaY > 0 ? "center top" : "center bottom")
            : (deltaX > 0 ? "left center" : "right center"),
        visible: true,
        phase: traveling ? "traveling" : "settled",
      });
      previousGeometry.current = next;

      if (traveling) {
        settleTimer.current = window.setTimeout(() => {
          setIndicator((current) => ({
            ...current,
            scaleX: 1,
            scaleY: 1,
            origin: "center",
            phase: "settled",
          }));
          settleTimer.current = null;
        }, 210);
      }
    };

    updateIndicator();
    window.addEventListener("resize", updateIndicator);
    const observer = typeof globalThis.ResizeObserver === "function"
      ? new ResizeObserver(updateIndicator)
      : null;
    observer?.observe(navElement);
    observer?.observe(activeElement);

    return () => {
      window.removeEventListener("resize", updateIndicator);
      observer?.disconnect();
      if (settleTimer.current !== null) {
        window.clearTimeout(settleTimer.current);
        settleTimer.current = null;
      }
    };
  }, [activeId, drawerOpen, isNarrow, items.length, settingsItem.id]);

  const indicatorStyle: CSSProperties = {
    width: indicator.width,
    height: indicator.height,
    opacity: indicator.visible ? 1 : 0,
    transform: `translate3d(${indicator.x}px, ${indicator.y}px, 0) scale3d(${indicator.scaleX}, ${indicator.scaleY}, 1)`,
    transformOrigin: indicator.origin,
  };
  const rootClassName = `cylune-nav sidebar${className ? ` ${className}` : ""}`;

  return <aside className={rootClassName} data-drawer-open={drawerOpen ? "true" : "false"}>
    <header className="cylune-nav__header">
      <div className="cylune-nav__brand" aria-label={brand.ariaLabel}>
        <span className="cylune-nav__brand-mark">{brand.mark}</span>
        <span className="cylune-nav__brand-copy">
          <h1>{brand.name}</h1>
          {brand.subtitle !== undefined ? <small>{brand.subtitle}</small> : null}
        </span>
      </div>
      <button
        ref={menuButton}
        type="button"
        className="cylune-nav__menu"
        aria-label={drawerOpen ? closeMenuLabel : menuLabel}
        aria-expanded={drawerOpen}
        aria-controls={drawerId}
        onClick={toggleDrawer}
      >
        <MenuIcon open={drawerOpen} />
      </button>
    </header>

    {isNarrow && drawerOpen ? <button
      type="button"
      className="cylune-nav__scrim"
      aria-label={closeMenuLabel}
      tabIndex={-1}
      data-testid="nav-drawer-scrim"
      onClick={closeDrawer}
    /> : null}

    <div
      ref={drawer}
      id={drawerId}
      className="cylune-nav__drawer"
      data-state={drawerOpen ? "open" : "closed"}
      role={isNarrow ? "dialog" : undefined}
      aria-modal={isNarrow && drawerOpen ? "true" : undefined}
      aria-label={isNarrow ? ariaLabel : undefined}
      aria-hidden={isNarrow && !drawerOpen ? "true" : undefined}
    >
      <nav ref={navigation} className="cylune-nav__navigation" aria-label={ariaLabel}>
        <span
          className="cylune-nav__indicator"
          data-phase={indicator.phase}
          data-testid="nav-active-indicator"
          aria-hidden="true"
          style={indicatorStyle}
        />
        <div className="cylune-nav__primary">
          {items.map((item, index) => <NavDestination
            key={item.id}
            item={item}
            active={item.id === activeId}
            tooltipId={`${idPrefix}-tooltip-${index}`}
            tabIndex={closedTabIndex}
            buttonRef={(element) => rememberItem(item.id, element)}
            onChoose={choose}
          />)}
        </div>

        {importAction ? <button
          type="button"
          className="cylune-nav__import"
          aria-label={importAction.label}
          disabled={importAction.disabled}
          tabIndex={closedTabIndex}
          onClick={runImport}
        >
          {importAction.icon !== undefined ? <span aria-hidden="true">{importAction.icon}</span> : null}
          <span>{importAction.label}</span>
        </button> : null}

        <div className="cylune-nav__footer">
          {privacy ? <div className="cylune-nav__privacy">
            <strong>{privacy.title}</strong>
            {privacy.description !== undefined ? <small>{privacy.description}</small> : null}
          </div> : null}
          <NavDestination
            item={settingsItem}
            active={settingsItem.id === activeId}
            tooltipId={`${idPrefix}-tooltip-settings`}
            tabIndex={closedTabIndex}
            buttonRef={(element) => rememberItem(settingsItem.id, element)}
            onChoose={choose}
          />
        </div>
      </nav>
    </div>
  </aside>;
}
