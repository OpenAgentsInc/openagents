import React from "react";
import { Link as RouterLink } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { react19 } from "@openagents/core";

// Define interface for router Link props
interface LinkProps {
  to: string;
  children?: React.ReactNode;
  [key: string]: any;
}

// Make React Router components compatible with React 19
const Link = react19.router<LinkProps>(RouterLink);
import {
  NavigationMenu as NavigationMenuBase,
  NavigationMenuItem,
  NavigationMenuLink,
  NavigationMenuList,
  navigationMenuTriggerStyle,
} from "../ui/navigation-menu";

export default function NavigationMenu() {
  const { t } = useTranslation();

  return (
    <NavigationMenuBase className="px-2 font-mono text-muted-foreground">
      <NavigationMenuList>
        <NavigationMenuItem>
          <Link to="/">
            <NavigationMenuLink className={navigationMenuTriggerStyle()}>
              {t("titleHomePage")}
            </NavigationMenuLink>
          </Link>
        </NavigationMenuItem>
        <NavigationMenuItem>
          <Link to="/second-page">
            <NavigationMenuLink className={navigationMenuTriggerStyle()}>
              {t("titleSecondPage")}
            </NavigationMenuLink>
          </Link>
        </NavigationMenuItem>
      </NavigationMenuList>
    </NavigationMenuBase>
  );
}
