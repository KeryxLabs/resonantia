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

EXPOSE 4173

CMD ["npm", "run", "preview:lan"]
