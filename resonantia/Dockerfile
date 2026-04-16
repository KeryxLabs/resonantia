# Runtime config is injected by docker-entrypoint.sh from environment variables.
# No VITE_* build args needed — the image is environment-agnostic.
FROM node:20-alpine AS build
WORKDIR /app

COPY package.json package-lock.json preamble.md ./
COPY svelte.config.js tsconfig.json vite.config.js ./
COPY static ./static
COPY src ./src
COPY packages ./packages

RUN npm ci
RUN npm run build

FROM node:20-alpine AS runtime
WORKDIR /app

ENV NODE_ENV=production

COPY --from=build /app /app
COPY docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

EXPOSE 4173

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["npm", "run", "preview:lan"]
