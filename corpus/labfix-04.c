/* ЛБ2, задача 1: цикл с полной проверкой ввода.
 * Сумма цифр натурального числа, повторный запрос при ошибке. */
#include <stdio.h>

#define BASE 10

int main(void) {
    int n = 0;
    int x = 0;
    int sum = 0;
    int ok = 0;

    while (!ok) {
        printf("натуральное число: ");
        if (scanf("%d", &n) != 1 || n <= 0) {
            printf("нужно целое больше нуля, повтори\n");
            while (getchar() != '\n') {
            }
        } else {
            ok = 1;
        }
    }
    x = n;
    do {
        sum = sum + x % BASE;
        x = x / BASE;
    } while (x > 0);
    printf("сумма цифр числа %d = %d\n", n, sum);
    return 0;
}
