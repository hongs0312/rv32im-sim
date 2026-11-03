#include "common.h"

// memset: 메모리 영역을 특정 값으로 초기화하는 함수 (주로  메모리를 0으로 초기화하는데 사용됨)
// buf: 초기화할 메모리 영역의 시작 주소
// c: 초기화할 값
// n: 초기화할 바이트 수
void *memset(void *buf, char c, size_t n) {
    uint8_t *p = (uint8_t *) buf;
    while (n--)
        *p++ = c;
    return buf;
}

void *memcpy(void *dst, const void *src, size_t n) {
    uint8_t *d = (uint8_t *) dst;
    const uint8_t *s = (const uint8_t *) src;
    while (n--)
        *d++ = *s++;
    return dst;
}

char *strcpy(char *dst, const char *src) {
    char *d = dst;
    while (*src)
        *d++ = *src++;
    *d = '\0';
    return dst;
}

int strcmp(const char *s1, const char *s2) {
    while (*s1 && *s2) {
        if (*s1 != *s2)
            break;
        s1++;
        s2++;
    }

    return *(unsigned char *)s1 - *(unsigned char *)s2;
}

void putchar(char ch);

void printf(const char *fmt, ...) {
    va_list vargs;
    va_start(vargs, fmt);

    while(*fmt) {
        if(*fmt == '%') {
            fmt++; // '%' 다음 문자를 확인
            switch(*fmt) {
                case '\0': // 문자열 끝에 도달하면 루프 종료
                    va_end(vargs);
                    goto end;
                case '%': // '%%'는 '%' 문자 출력
                    putchar('%');
                    break;
                case 's': { // /0-terminated string 출력
                    const char *s = va_arg(vargs, const char *);
                    while(*s) {
                        putchar(*s);
                        s++;
                    }
                    break;
                }
                case 'd': { // signed decimal integer 출력
                    int value = va_arg(vargs, int);
                    unsigned magnitude = value;
                    if(value < 0) {
                        putchar('-');
                        magnitude = -magnitude;
                    }

                    unsigned divisor = 1;
                    while(magnitude / divisor > 9)
                        divisor *= 10;

                    while(divisor > 0) {
                        putchar('0' + magnitude / divisor);
                        magnitude %= divisor;
                        divisor /= 10;
                    }

                    break;
                }
                case 'x': { // unsigned hexadecimal integer 출력
                    unsigned value = va_arg(vargs, unsigned);
                    for(int i = 7; i >= 0; i--) {
                        unsigned nibble = (value >> (i * 4)) & 0xf;
                        putchar("0123456789abcdef"[nibble]);
                    }
                }
            }
        } else {
            putchar(*fmt);
        }

        fmt++;
    }
end:
    va_end(vargs);
}
