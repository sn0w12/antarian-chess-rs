import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { loadName, saveName } from "@/lib/name";

export function MenuPage() {
  const navigate = useNavigate();
  const [name, setName] = useState(loadName);
  const trimmed = name.trim();
  const nameEmpty = trimmed.length === 0;

  const remember = () => {
    saveName(trimmed);
  };

  const startBot = () => {
    remember();
    navigate("/bot");
  };

  const startOnline = (intent: string) => {
    remember();
    navigate(`/online?intent=${intent}`);
  };

  return (
    <main className="flex h-dvh items-center justify-center overflow-hidden p-4">
      <Card className="max-h-full w-full max-w-[440px] [@media(max-height:560px)]:max-w-[660px]">
        <CardPanel className="flex min-h-0 flex-col gap-4 overflow-y-auto p-4 [@media(max-height:560px)]:grid [@media(max-height:560px)]:grid-cols-2 [@media(max-height:560px)]:gap-x-6">
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1">
              <h1 className="font-heading text-xl font-semibold">
                Antarian Chess
              </h1>
              <p className="text-sm text-muted-foreground">
                Play local, queue randomly, or host your own match.
              </p>
            </div>

            <Field>
              <FieldLabel>Player Name</FieldLabel>
              <Input
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={24}
              />
            </Field>

            <div className="flex flex-col gap-2">
              <span className="text-sm text-muted-foreground">Solo</span>
              <Button size="lg" className="w-full" onClick={startBot}>
                vs Bot
              </Button>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <Separator className="[@media(max-height:560px)]:hidden" />

            <span className="text-sm text-muted-foreground">Online</span>
            <Button
              size="lg"
              className="w-full"
              disabled={nameEmpty}
              onClick={() => startOnline("random")}
            >
              Random Online
            </Button>
            <Button
              className="w-full"
              disabled={nameEmpty}
              onClick={() => startOnline("browse")}
            >
              Browse Lobbies
            </Button>
            <div className="flex flex-col gap-2 lg:flex-row">
              <Separator className="lg:hidden" />
              <Button
                className="lg:flex-1"
                disabled={nameEmpty}
                onClick={() => startOnline("public")}
              >
                Create Public Lobby
              </Button>
              <Button
                className="lg:flex-1"
                disabled={nameEmpty}
                onClick={() => startOnline("private")}
              >
                Create Private Lobby
              </Button>
            </div>
          </div>
        </CardPanel>
      </Card>
    </main>
  );
}
