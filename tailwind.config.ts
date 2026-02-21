import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        sarah: {
          bg: "#0B0F1A",
          cyan: "#22D3EE",
          violet: "#A78BFA",
          amber: "#FBBF24",
        },
      },
      boxShadow: {
        "sarah-orb": "0 0 80px rgba(56, 189, 248, 0.35)",
      },
    },
  },
};

export default config;
