import { useState, type MouseEvent } from "react";
import { DiscordLogoIcon } from "@phosphor-icons/react/dist/csr/DiscordLogo";
import s from "./DiscordLink.module.css";

export const DISCORD_URL = "https://discord.gg/beyMx7uuTX";

export function DiscordLink() {
  const [failed, setFailed] = useState(false);
  async function open(event: MouseEvent<HTMLAnchorElement>) {
    if (!("__TAURI_INTERNALS__" in window)) return;
    event.preventDefault();
    setFailed(false);
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(DISCORD_URL);
    } catch { setFailed(true); }
  }
  return <span className={s.wrap}>
    <a className={s.link} href={DISCORD_URL} target="_blank" rel="noopener noreferrer" title="Discord de Panas RPG" aria-label="Abrir Discord de Panas RPG" onClick={event => void open(event)}><DiscordLogoIcon size={23} weight="fill" aria-hidden /></a>
    {failed && <span role="alert" className={s.error}>No se pudo abrir Discord. Copia este enlace en tu navegador: <span>{DISCORD_URL}</span><button type="button" onClick={() => setFailed(false)}>Cerrar</button></span>}
  </span>;
}
