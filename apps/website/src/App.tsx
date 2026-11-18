import { Link, RouterProvider, createBrowserRouter } from "react-router-dom";
import { MenuPage } from "@/pages/MenuPage";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";

function RouteFallback() {
  return (
    <main className="flex min-h-screen items-center justify-center">
      <Spinner aria-label="Loading page" />
    </main>
  );
}

function NotFoundPage() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 px-6 text-center">
      <p className="font-mono text-sm text-muted-foreground">404</p>
      <h1 className="font-heading text-3xl font-semibold tracking-tight">
        That page does not exist
      </h1>
      <Button render={<Link to="/" />} variant="outline">
        Back to menu
      </Button>
    </main>
  );
}

const router = createBrowserRouter([
  { path: "/", element: <MenuPage /> },
  {
    path: "/bot",
    lazy: async () => ({ Component: (await import("@/pages/BotPage")).BotPage }),
    hydrateFallbackElement: <RouteFallback />,
  },
  {
    path: "/online",
    lazy: async () => ({
      Component: (await import("@/pages/OnlinePage")).OnlinePage,
    }),
    hydrateFallbackElement: <RouteFallback />,
  },
  { path: "*", element: <NotFoundPage /> },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
